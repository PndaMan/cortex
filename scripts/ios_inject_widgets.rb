#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Injects the CortexWidgets app-extension target into the Tauri-generated iOS Xcode project.
# Tauri regenerates `src-tauri/gen/apple` on every `cargo tauri ios init`, and Tauri has no notion
# of app extensions — so this runs in CI AFTER `ios init` and BEFORE the build/sign steps to:
#   1. create the `CortexWidgets` app-extension target (the 8 widgets + recording Live Activity),
#   2. add the committed widget Swift sources (referenced in place, no copying),
#   3. link the shared `CortexShared` local Swift package to the extension (so the
#      RecordingAttributes type identity is shared with the app, which links it via the plugin),
#   4. embed the extension into the app ("Embed App Extensions" copy-files phase),
#   5. point the app target at the App-Group entitlements + raise its deployment target to iOS 16,
#   6. set the extension's bundle id / Info.plist / entitlements / deployment target.
#
# Requires the `xcodeproj` gem (preinstalled on GitHub macOS runners via CocoaPods).
#
# Usage: ruby scripts/ios_inject_widgets.rb [path/to/Project.xcodeproj]

require 'xcodeproj'
require 'pathname'

APP_BUNDLE_ID    = 'study.cortex.app'
WIDGET_BUNDLE_ID = 'study.cortex.app.CortexWidgets'
EXT_NAME         = 'CortexWidgets'
DEPLOYMENT       = '16.0'
SWIFT_VERSION    = '5.0'

repo_root   = Pathname.new(File.expand_path('..', __dir__))
widgets_dir = repo_root.join('src-tauri/ios-widgets/CortexWidgets')
ext_plist   = repo_root.join('src-tauri/ios-widgets/CortexWidgets/Info.plist')
ext_entl    = repo_root.join('src-tauri/ios-widgets/CortexWidgets/CortexWidgets.entitlements')
app_entl    = repo_root.join('src-tauri/ios-widgets/CortexApp.entitlements')

proj_path = ARGV[0] || Dir.glob(repo_root.join('src-tauri/gen/apple/*.xcodeproj').to_s).first
abort("❌ no .xcodeproj found under src-tauri/gen/apple — run `cargo tauri ios init` first") unless proj_path && File.exist?(proj_path)
puts "→ project: #{proj_path}"

project   = Xcodeproj::Project.open(proj_path)
proj_dir  = Pathname.new(File.dirname(File.expand_path(proj_path)))

def rel(path, from)
  Pathname.new(File.expand_path(path)).relative_path_from(from).to_s
end

app_target = project.targets.find { |t| t.product_type == 'com.apple.product-type.application' }
abort('❌ could not find the application target') unless app_target
puts "→ app target: #{app_target.name}"

# Idempotency: bail cleanly if we've already injected (CI may re-run).
if project.targets.any? { |t| t.name == EXT_NAME }
  puts "✓ #{EXT_NAME} target already present — skipping injection"
  exit 0
end

# 1) Create the app-extension target.
ext = project.new_target(:app_extension, EXT_NAME, :ios, DEPLOYMENT, nil, :swift)
puts "→ created extension target: #{ext.name}"

# 2) Add the committed widget sources (referenced in place via a relative path).
group = project.main_group.find_subpath(EXT_NAME, true)
group.set_source_tree('SOURCE_ROOT')
swift_files = Dir.glob(widgets_dir.join('*.swift').to_s).sort
abort("❌ no widget Swift sources found in #{widgets_dir}") if swift_files.empty?
swift_files.each do |f|
  ref = group.new_reference(rel(f, proj_dir))
  ref.source_tree = 'SOURCE_ROOT'
  ext.source_build_phase.add_file_reference(ref, true)
  puts "   + #{File.basename(f)}"
end

# 3) Link the shared CortexShared local Swift package to the extension.
local_ref_class = Xcodeproj::Project::Object.const_get(:XCLocalSwiftPackageReference) rescue nil
abort('❌ this xcodeproj gem is too old for local SPM refs (needs >= 1.22). Update CocoaPods.') unless local_ref_class
pkg = project.root_object.package_references.find do |r|
  r.respond_to?(:relative_path) && r.relative_path.to_s.include?('CortexShared')
end
unless pkg
  pkg = project.new(local_ref_class)
  pkg.relative_path = '../../ios/CortexShared'
  project.root_object.package_references ||= []
  project.root_object.package_references << pkg
end
dep = project.new(Xcodeproj::Project::Object::XCSwiftPackageProductDependency)
dep.product_name = 'CortexShared'
dep.package = pkg
ext.package_product_dependencies << dep
bf = project.new(Xcodeproj::Project::Object::PBXBuildFile)
bf.product_ref = dep
ext.frameworks_build_phase.files << bf
puts '→ linked CortexShared package to the extension'

# 4) Embed the extension into the app + add a target dependency.
app_target.add_dependency(ext)
embed = app_target.copy_files_build_phases.find { |p| p.name == 'Embed App Extensions' }
unless embed
  embed = app_target.new_copy_files_build_phase('Embed App Extensions')
  embed.symbol_dst_subfolder_spec = :plug_ins
end
embed_bf = embed.add_file_reference(ext.product_reference)
embed_bf.settings = { 'ATTRIBUTES' => ['RemoveHeadersOnCopy'] }
puts '→ embedded extension into the app'

# 5) Extension build settings.
ext.build_configurations.each do |cfg|
  s = cfg.build_settings
  s['PRODUCT_BUNDLE_IDENTIFIER']   = WIDGET_BUNDLE_ID
  s['PRODUCT_NAME']                = '$(TARGET_NAME)'
  s['INFOPLIST_FILE']              = rel(ext_plist, proj_dir)
  s['CODE_SIGN_ENTITLEMENTS']      = rel(ext_entl, proj_dir)
  s['IPHONEOS_DEPLOYMENT_TARGET']  = DEPLOYMENT
  s['SWIFT_VERSION']               = SWIFT_VERSION
  s['TARGETED_DEVICE_FAMILY']      = '1,2'
  s['GENERATE_INFOPLIST_FILE']     = 'NO'
  s['SKIP_INSTALL']                = 'YES'
  s['CODE_SIGN_STYLE']             = 'Automatic'
  s['SWIFT_EMIT_LOC_STRINGS']      = 'YES'
  s['MARKETING_VERSION']           = '1.0'
  s['CURRENT_PROJECT_VERSION']     = '1'
  s['LD_RUNPATH_SEARCH_PATHS']     = ['$(inherited)', '@executable_path/Frameworks', '@executable_path/../../Frameworks']
end

# 6) App target: App-Group entitlements + raise deployment target to iOS 16 (CortexShared floor).
app_target.build_configurations.each do |cfg|
  s = cfg.build_settings
  existing = s['CODE_SIGN_ENTITLEMENTS']
  if existing.nil? || existing.to_s.strip.empty?
    s['CODE_SIGN_ENTITLEMENTS'] = rel(app_entl, proj_dir)
  else
    puts "ℹ︎ app already has entitlements (#{existing}); ensure it contains group.study.cortex.app"
  end
  cur = s['IPHONEOS_DEPLOYMENT_TARGET'].to_f
  s['IPHONEOS_DEPLOYMENT_TARGET'] = DEPLOYMENT if cur.zero? || cur < DEPLOYMENT.to_f
end

project.save
puts "✅ injected #{EXT_NAME} extension into #{File.basename(proj_path)}"
puts '   targets now: ' + project.targets.map(&:name).join(', ')
