if ! RUSTFLAGS="-Ctarget-cpu=native" RUSTC_BOOTSTRAP="qcms" cargo build --release ; then
    echo "Build failed -> exiting"
    exit
fi

echo "Installing"

if ! install -C -D -t $HOME/.local/bin/ ./target/release/avis-imgv ; then
    echo "Installation failed -> exiting"
fi

echo "Installation complete"

config_dir=$HOME/.config/avis-imgv
applications_dir=$HOME/.local/share/applications

if [ ! -f $config_dir/config.yaml ]; then
    echo "Configuration doesn't exist yet -> creating base configuration"
    mkdir -p $config_dir  
    cp ./examples/config.yaml $config_dir/config.yaml
fi

mkdir -p $applications_dir

if [ ! -f $applications_dir/avis-imgv.desktop ]; then
    echo "
    [Desktop Entry]
    Exec=$HOME/.local/bin/avis-imgv
    MimeType=image/png;image/jpeg;image/jpg;image/webp;
    Name=AvisImgv
    NoDisplay=true
    Type=Application
    " > $applications_dir/avis-imgv.desktop 

    echo "Updating desktop database"
    update-desktop-database -v $applications_dir
fi
 

