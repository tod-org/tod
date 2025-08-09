require "digest"
require "open-uri"

version = ARGV[0] or abort("Usage: ruby update_formula.rb <version> (e.g. 0.8.0)")
tag = "v#{version}"
base_url = "https://github.com/tod-org/tod/releases/download/#{tag}"

platforms = {
  mac_arm:     "tod-#{version}-darwin-arm64.tar.gz",
  mac_intel:   "tod-#{version}-darwin-amd64.tar.gz",
  linux_arm:   "tod-#{version}-linux-arm64.tar.gz",
  linux_intel: "tod-#{version}-linux-amd64.tar.gz"
}

sha256s = {}

# Download and compute SHA256s
platforms.each do |key, filename|
  url = "#{base_url}/#{filename}"
  begin
    puts "🔽 Downloading #{url}..."
    file = URI.open(url).read
    sha256s[key] = Digest::SHA256.hexdigest(file)
    puts "✅ SHA256 for #{key}: #{sha256s[key]}"
  rescue => e
    warn "❌ Failed to download or hash #{url}: #{e.message}"
    exit 1
  end
end

formula_path = "Formula/tod.rb"
formula = File.read(formula_path)

# Track whether all updates succeeded
failures = []

# Version update with logging
old_version = formula[/version\s+"([^"]+)"/, 1]
if old_version == version
  puts "ℹ️ Version remains unchanged at #{version}"
else
  puts "🔄 Updating version: #{old_version} → #{version}"
end
formula.gsub!(/version\s+"[^"]+"/, "version \"#{version}\"")

# Helper to replace URL and SHA within a platform-specific block
def replace_platform_block(formula, platform_key, filename, new_sha, version, failures)
  platform_section = case platform_key
  when :mac_arm     then [/on_macos.*?on_arm.*?\n(.*?)url\s+"[^"]+"\n\s+sha256\s+"[a-f0-9]+"/m, "macOS ARM"]
  when :mac_intel   then [/on_macos.*?on_intel.*?\n(.*?)url\s+"[^"]+"\n\s+sha256\s+"[a-f0-9]+"/m, "macOS Intel"]
  when :linux_arm   then [/on_linux.*?on_arm.*?\n(.*?)url\s+"[^"]+"\n\s+sha256\s+"[a-f0-9]+"/m, "Linux ARM"]
  when :linux_intel then [/on_linux.*?on_intel.*?\n(.*?)url\s+"[^"]+"\n\s+sha256\s+"[a-f0-9]+"/m, "Linux Intel"]
  end

  pattern, label = platform_section
  new_url = "url \"https://github.com/tod-org/tod/releases/download/v#{version}/#{filename}\""
  new_sha_line = "sha256 \"#{new_sha}\""

  updated = formula.sub(pattern) do |block|
    block.gsub(/url\s+"[^"]+"/, new_url).gsub(/sha256\s+"[a-f0-9]+"/, new_sha_line)
  end

  if updated == formula
    warn "❌ Could not find or replace block for #{label}"
    failures << label
  else
    puts "✅ Updated #{label} block"
    formula.replace(updated)
  end
end

# Replace each platform-specific block
platforms.each do |key, filename|
  replace_platform_block(formula, key, filename, sha256s[key], version, failures)
end

# Fail CI if any block was not updated
if failures.any?
  warn "\n❌ The following blocks failed to update: #{failures.join(', ')}"
  exit 1
end

# Save changes
File.write(formula_path, formula)
puts "\n✅ All platform blocks updated successfully"
puts "📄 Wrote updated Formula/tod.rb for v#{version}"
