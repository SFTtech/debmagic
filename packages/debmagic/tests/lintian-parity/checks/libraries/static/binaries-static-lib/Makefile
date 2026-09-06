CC=gcc
CFLAGS+= -g
LIBFILES:= libfoo.a libbar.a

all: $(LIBFILES)

libfoo.a: code.o other.o
	ar rcs -o $@ $^

pstripped.o: other.o
	cp other.o pstripped.o
	strip --strip-unneeded pstripped.o

fstripped.o: other.o
	cp other.o fstripped.o
	strip --strip-unneeded --remove-section=.comment \
	    --remove-section=.note fstripped.o

libbar.a: fstripped.o pstripped.o
	ar rcs -o $@ $^

clean:
	rm -f *.a *.o

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0755 -d $(DESTDIR)/usr/include
	install -m 0644 *.a $(DESTDIR)/usr/lib
	install -m 0644 code.h $(DESTDIR)/usr/include

.PHONY: install clean
