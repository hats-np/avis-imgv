#!/bin/bash
xmp=$1.RAF.xmp #hardcoded for fuji but can be made to search a variety of extensions with little work. 

if [ -f "$xmp" ]; then 
  sed -i "s/xmp:Rating=\".\"/xmp:Rating=\"$2\"/" $xmp
else
  echo "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"XMP Core 4.4.0-Exiv2\">
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">
  <rdf:Description rdf:about=\"\"
    xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"
    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"
    xmlns:xmpMM=\"http://ns.adobe.com/xap/1.0/mm/\"
    xmp:Rating=\"$2\">
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>" > $xmp
fi
