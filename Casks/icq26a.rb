cask "icq26a" do
  version "0.1.2"

  on_arm do
    url "https://github.com/fredsmith/icq26a/releases/download/v#{version}/ICQ26a_#{version}_aarch64.dmg"
    sha256 "PLACEHOLDER" # :arm64
  end
  on_intel do
    url "https://github.com/fredsmith/icq26a/releases/download/v#{version}/ICQ26a_#{version}_x64.dmg"
    sha256 "PLACEHOLDER" # :x64
  end

  name "ICQ26a"
  desc "Matrix chat client styled after ICQ 98a"
  homepage "https://github.com/fredsmith/icq26a"

  app "ICQ26a.app"

  caveats <<~EOS
    #{token} is not signed with an Apple Developer certificate.
    On first launch, macOS Gatekeeper will block it. To allow it:
      System Settings > Privacy & Security > scroll down > click "Open Anyway"
    Or run:
      xattr -d com.apple.quarantine /Applications/ICQ26a.app
  EOS

  zap trash: ["~/Library/Application Support/com.icq26a.app"]
end
