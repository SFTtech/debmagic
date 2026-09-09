CC=gcc
CFLAGS+= -g
LIBFILES:= librealname_s.a

all: $(LIBFILES)

librealname_s.a: code.o
	ar rcs -o $@ $^

clean:
	rm -f *.a *.o

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0644 *.a $(DESTDIR)/usr/lib

.PHONY: install clean
