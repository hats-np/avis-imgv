#!/bin/bash
#Converted into png first since not all apps accept jpgs or other formats
convert $1 png:- | wl-copy
