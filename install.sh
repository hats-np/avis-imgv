RUSTC_BOOTSTRAP="qcms" cargo build --release

old_bin="$HOME/.local/bin/avis-imgv"
if [ -f "$old_bin" ] ; then
    echo "Deleting old binary"
    rm "$old_bin"
fi

echo "Copying new binary"
cp ./target/release/avis-imgv $HOME/.local/bin

echo "
[Desktop Entry]
Exec=$HOME/.local/bin/avis-imgv
MimeType=image/png;image/jpeg;image/jpg;image/webp;
Name=AvisImgv
NoDisplay=true
Type=Application
" > $HOME/.local/share/applications/avis-imgv.desktop 

echo "Updating desktop database"
update-desktop-database -v ~/.local/share/applications/
