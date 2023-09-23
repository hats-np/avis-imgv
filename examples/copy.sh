#!/bin/bash

#Requires imagemagick, wl-copy for wayland or xclip for x11
#Converted into png first since not all apps accept jpgs or other formats

#Wayland
convert $1 png:- | wl-copy

#X11
convert $1 png:- | xclip -selection clipboard -target image/png -i

#If you use both regularly
if [ $XDG_SESSION_TYPE == "x11" ] 
then
        convert $1 png:- | xclip -selection clipboard -target image/png -i

else 
        convert $1 png:- | wl-copy
fi