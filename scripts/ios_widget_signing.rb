#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Re-signs ONLY the CortexWidgets extension target with its own App Store provisioning profile.
# The app's "Force manual App Store signing" perl patch sets every target to the APP profile;
# an app extension needs its OWN profile (bundle id study.cortex.app.CortexWidgets), so this runs
# after that patch to repoint just the extension. Reads WIDGET_PROFILE_NAME / APPLE_TEAM_ID /
# SIGN_IDENTITY from the environment.

require 'xcodeproj'

proj_path = ARGV[0] || Dir.glob('src-tauri/gen/apple/*.xcodeproj').first
abort('❌ no .xcodeproj found') unless proj_path && File.exist?(proj_path)

project = Xcodeproj::Project.open(proj_path)
ext = project.targets.find { |t| t.name == 'CortexWidgets' }
abort('❌ CortexWidgets target not found') unless ext

profile = ENV.fetch('WIDGET_PROFILE_NAME')
team    = ENV.fetch('APPLE_TEAM_ID')
ident   = ENV.fetch('SIGN_IDENTITY')

ext.build_configurations.each do |c|
  s = c.build_settings
  s['CODE_SIGN_STYLE'] = 'Manual'
  s['DEVELOPMENT_TEAM'] = team
  s['PROVISIONING_PROFILE_SPECIFIER'] = profile
  s.delete('PROVISIONING_PROFILE')
  s['CODE_SIGN_IDENTITY'] = ident
  s['CODE_SIGN_IDENTITY[sdk=iphoneos*]'] = ident
end

project.save
puts "✅ CortexWidgets signed with profile '#{profile}' (#{ident})"
