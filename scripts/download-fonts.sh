#!/usr/bin/env bash
set -euo pipefail

DEST_DIR="fonts"
FORCE="0"
TLWG_OTF_ARCHIVE_URL="https://github.com/tlwg/fonts-tlwg/releases/download/v0.7.3/otf-tlwg-0.7.3.tar.xz"

usage() {
  cat <<'EOF'
Download runtime fonts used by LynPDF examples/tests.

Includes:
- Google/emoji fonts used by fixtures
- TLWG OTF bundle under tlwg/otf/

Usage:
  ./scripts/download-fonts.sh [--dest DIR] [--force]

Options:
  --dest DIR   Destination directory (default: fonts)
  --force      Re-download even if target file already exists
  -h, --help   Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dest)
      [[ $# -lt 2 ]] && { echo "error: --dest requires a value" >&2; exit 1; }
      DEST_DIR="$2"
      shift 2
      ;;
    --force)
      FORCE="1"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required" >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "error: tar is required" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"

# filename|url
FONT_SPECS=(
  "Sarabun-Regular.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-Regular.ttf"
  "Sarabun-Bold.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-Bold.ttf"
  "Sarabun-Italic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-Italic.ttf"
  "Sarabun-BoldItalic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-BoldItalic.ttf"
  "Prompt-Regular.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-Regular.ttf"
  "Prompt-Bold.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-Bold.ttf"
  "Prompt-Italic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-Italic.ttf"
  "Prompt-BoldItalic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-BoldItalic.ttf"
  "Kanit-Regular.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-Regular.ttf"
  "Kanit-Bold.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-Bold.ttf"
  "Kanit-Italic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-Italic.ttf"
  "Kanit-BoldItalic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-BoldItalic.ttf"
  "Mitr-Regular.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/mitr/Mitr-Regular.ttf"
  "Mitr-Bold.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/mitr/Mitr-Bold.ttf"
  "ChakraPetch-Regular.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-Regular.ttf"
  "ChakraPetch-Bold.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-Bold.ttf"
  "ChakraPetch-Italic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-Italic.ttf"
  "ChakraPetch-BoldItalic.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-BoldItalic.ttf"
  "IBMPlexSans-Variable.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/ibmplexsans/IBMPlexSans%5Bwdth%2Cwght%5D.ttf"
  "InterVariable.ttf|https://raw.githubusercontent.com/google/fonts/main/ofl/inter/Inter%5Bopsz%2Cwght%5D.ttf"
  "NotoColorEmoji.ttf|https://github.com/googlefonts/noto-emoji/raw/main/fonts/NotoColorEmoji.ttf"
)

download_one() {
  local file_name="$1"
  local url="$2"
  local out_path="$DEST_DIR/$file_name"

  if [[ -f "$out_path" && "$FORCE" != "1" ]]; then
    echo "skip  $file_name"
    return 0
  fi

  echo "fetch $file_name"
  curl -fsSL --retry 3 --retry-delay 1 "$url" -o "$out_path"
}

download_tlwg_otf_bundle() {
  local tlwg_dir="$DEST_DIR/tlwg/otf"
  mkdir -p "$tlwg_dir"

  if [[ "$FORCE" != "1" ]] && find "$tlwg_dir" -type f -name '*.otf' | grep -q .; then
    echo "skip  tlwg-otf bundle"
    return 0
  fi

  local tmp_dir
  tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/lynpdf-rs-fonts.XXXXXX")"
  local archive_path="$tmp_dir/otf-tlwg-0.7.3.tar.xz"

  echo "fetch tlwg-otf bundle"
  curl -fsSL --retry 3 --retry-delay 1 "$TLWG_OTF_ARCHIVE_URL" -o "$archive_path"

  tar -xJf "$archive_path" -C "$tmp_dir"

  local extracted_count=0
  while IFS= read -r -d '' font_file; do
    cp "$font_file" "$tlwg_dir/"
    extracted_count=$((extracted_count + 1))
  done < <(find "$tmp_dir" -type f -name '*.otf' -print0)

  rm -rf "$tmp_dir"

  if [[ "$extracted_count" -eq 0 ]]; then
    echo "error: TLWG OTF bundle downloaded but no .otf files were found" >&2
    exit 1
  fi

  echo "done  tlwg-otf ($extracted_count files)"
}

for spec in "${FONT_SPECS[@]}"; do
  file_name="${spec%%|*}"
  url="${spec#*|}"
  download_one "$file_name" "$url"
done

download_tlwg_otf_bundle

echo "done: fonts downloaded to $DEST_DIR"
