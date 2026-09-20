all:
	GO111MODULE=on go build -o use-godebug main.go

install:
	install -d $(DESTDIR)/usr/bin/
	install -m 644 use-godebug $(DESTDIR)/usr/bin/use-godebug

clean distclean:
	rm -f use-godebug
