#!/bin/bash
# Installs Mandarina, its Finder Quick Action and (optionally) the converters it uses.
set -e
cd "$(dirname "$0")"
SUPPORT="$PWD/.support"

echo ""
echo "  🍊 Mandarina installer for macOS"
echo ""

# 1. The app: /Applications if writable, otherwise ~/Applications
DEST=/Applications
[ -w "$DEST" ] || { DEST="$HOME/Applications"; mkdir -p "$DEST"; }
echo "→ Installing Mandarina.app to $DEST"
rm -rf "$DEST/Mandarina.app"
ditto "Mandarina.app" "$DEST/Mandarina.app"
# The app is ad-hoc signed, not signed with an Apple Developer ID or notarized. Files downloaded
# from the internet carry the com.apple.quarantine flag, and Gatekeeper refuses to open such an
# app ("damaged" / "unidentified developer"). Removing the flag from this one copy is what
# "Open Anyway" in System Settings would do. See docs/macOS.md.
xattr -dr com.apple.quarantine "$DEST/Mandarina.app" 2>/dev/null || true
# Register this copy so the Quick Action (which opens the app by its id) finds it
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$DEST/Mandarina.app" 2>/dev/null || true

# 2. The Finder right-click menu item
echo "→ Adding Finder Quick Action \"Convert with Mandarina\""
mkdir -p "$HOME/Library/Services"
rm -rf "$HOME/Library/Services/Convert with Mandarina.workflow"
ditto "$SUPPORT/Convert with Mandarina.workflow" "$HOME/Library/Services/Convert with Mandarina.workflow"
xattr -dr com.apple.quarantine "$HOME/Library/Services/Convert with Mandarina.workflow" 2>/dev/null || true
/System/Library/CoreServices/pbs -update 2>/dev/null || true

# 3. Converters (Mandarina drives these tools; each one unlocks some formats)
for d in /usr/local/bin /opt/homebrew/bin /usr/local/opt/ffmpeg-full/bin /opt/homebrew/opt/ffmpeg-full/bin; do [ -d "$d" ] && PATH="$d:$PATH"; done
missing=()
have() { command -v "$1" >/dev/null 2>&1; }
# Homebrew's plain ffmpeg has no Vorbis encoder (OGG); ffmpeg-full has every codec Mandarina uses
{ have ffmpeg && ffmpeg -hide_banner -encoders 2>/dev/null | grep -q libvorbis; } || missing+=(ffmpeg-full)
have magick || missing+=(imagemagick)
have pandoc || missing+=(pandoc)
have pdftoppm || missing+=(poppler)
have gs || missing+=(ghostscript)
have rsvg-convert || missing+=(librsvg)
have 7zz || have 7z || missing+=(sevenzip)
need_lo=0
[ -d /Applications/LibreOffice.app ] || [ -d "$HOME/Applications/LibreOffice.app" ] || need_lo=1

echo ""
if [ ${#missing[@]} -eq 0 ] && [ $need_lo -eq 0 ]; then
    echo "✓ All converters are already installed."
elif have brew; then
    echo "Some converters are missing: ${missing[*]}$([ $need_lo -eq 1 ] && echo " libreoffice")"
    echo "  ffmpeg-full = audio/video · imagemagick = images · pandoc = documents"
    echo "  poppler/ghostscript = PDF · librsvg = SVG · sevenzip = RAR · libreoffice = Office files"
    read -r -p "Install them now with Homebrew? [Y/n] " answer
    if [[ ! "$answer" =~ ^[Nn] ]]; then
        [ ${#missing[@]} -gt 0 ] && brew install "${missing[@]}"
        [ $need_lo -eq 1 ] && brew install --cask libreoffice
    fi
else
    echo "Mandarina needs a few free converters, installed with Homebrew (https://brew.sh)."
    echo "Install Homebrew, then run:"
    echo ""
    echo "  brew install ffmpeg-full imagemagick pandoc poppler ghostscript librsvg sevenzip"
    echo "  brew install --cask libreoffice"
    echo ""
    echo "(Without them Mandarina still opens, and tells you which tool a format needs.)"
fi

echo ""
echo "✓ Done! Right-click any file in Finder → Quick Actions → Convert with Mandarina."
echo "  Tip: give it a shortcut in System Settings → Keyboard → Keyboard Shortcuts →"
echo "  Services → Files and Folders → Convert with Mandarina."
echo ""
