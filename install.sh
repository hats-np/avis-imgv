if ! which cmake >/dev/null 2>&1; then
  echo "Please install cmake"
  exit
fi

if ! RUSTFLAGS="-C target-cpu=native" cargo build --release; then
  echo "Build failed -> exiting"
  exit
fi

echo "Installing"

if ! install -C -D -t $HOME/.local/bin/ ./target/release/avis-imgv; then
  echo "Installation failed -> exiting"
fi

echo "Installation complete"

applications_dir=$HOME/.local/share/applications

mkdir -p $applications_dir

if [ ! -f $applications_dir/avis-imgv.desktop ]; then
  echo "[Desktop Entry]
Exec=$HOME/.local/bin/avis-imgv
MimeType=image/png;image/jpeg;image/jpg;image/webp;
Name=Avis Image Viewer
NoDisplay=false
Type=Application" >$applications_dir/avis-imgv.desktop

  echo "Updating desktop database"
  update-desktop-database -v $applications_dir
fi
