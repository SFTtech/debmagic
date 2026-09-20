multiarch := $(shell dpkg-architecture -qDEB_HOST_MULTIARCH)

all:
	install -d usr/lib/$(multiarch)/girepository-1.0
	cp usr/lib/libgood-42-dev-dummy usr/lib/$(multiarch)/libperfect-42-dev-dummy
	cp usr/lib/libgood-42-0-dummy usr/lib/$(multiarch)/libperfect-42-0-dummy
	cp usr/lib/girepository-1.0/Good-42.typelib usr/lib/$(multiarch)/girepository-1.0/GoodExtras-42.typelib
	cp usr/lib/girepository-1.0/Good-42.typelib usr/lib/$(multiarch)/girepository-1.0/Perfect-42.typelib
	cp usr/lib/girepository-1.0/Bad-23.typelib usr/lib/$(multiarch)/girepository-1.0/BadExtras-23.typelib
	install -d usr/lib/$(multiarch)/gir-1.0
	cp usr/share/gir-1.0/Bad-23.gir usr/lib/$(multiarch)/gir-1.0/BadExtras-23.gir
	cp usr/share/gir-1.0/Good-42.gir usr/lib/$(multiarch)/gir-1.0/GoodExtras-42.gir
