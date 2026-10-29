#!/usr/bin/make -f

all: debian/config

debian/config: debian/templates
	perl config-gen.pl < $< > $@


