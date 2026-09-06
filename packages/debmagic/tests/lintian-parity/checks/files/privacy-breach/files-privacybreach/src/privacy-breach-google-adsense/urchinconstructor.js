var re = /loaded|complete/,
gaHost = ("https:" == window.doc.location.protocol) ? "https://ssl." : "http://www.",
h = window.doc.getElementsByTagName("head")[0],
n = construct.create('script', {
src: gaHost + 
"google-analytics.com/ga.js"
}, h);