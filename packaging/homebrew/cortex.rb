# Homebrew Cask for Cortex — install into your own tap:
#   1. Create a repo named `homebrew-cortex` (e.g. github.com/PndaMan/homebrew-cortex)
#   2. Put this file at Casks/cortex.rb
#   3. Users: `brew install --cask pndaman/cortex/cortex`
# Bump `version` + both sha256 on each release:
#   shasum -a 256 Cortex_1.0.1_aarch64.dmg
cask "cortex" do
  version "1.0.6"

  on_arm do
    sha256 "REPLACE_WITH_AARCH64_DMG_SHA256"
    url "https://github.com/PndaMan/cortex/releases/download/v#{version}/Cortex_#{version}_aarch64.dmg"
  end
  on_intel do
    sha256 "REPLACE_WITH_X64_DMG_SHA256"
    url "https://github.com/PndaMan/cortex/releases/download/v#{version}/Cortex_#{version}_x64.dmg"
  end

  name "Cortex"
  desc "Local-first, open-source NotebookLM alternative — a desktop study OS"
  homepage "https://github.com/PndaMan/cortex"

  app "Cortex.app"

  # Until the app is notarized, Gatekeeper quarantines it. Either notarize (see
  # SIGNING.md) or tell users to clear the quarantine flag once after install:
  #   xattr -dr com.apple.quarantine "/Applications/Cortex.app"
  zap trash: [
    "~/Library/Application Support/study.cortex.app",
    "~/Library/Caches/study.cortex.app",
  ]
end
