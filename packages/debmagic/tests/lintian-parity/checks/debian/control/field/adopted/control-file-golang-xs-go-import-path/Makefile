NOPIE_CFLAGS = $(filter-out -fPIE,$(CFLAGS))
NOPIE_LDFLAGS = $(filter-out -fPIE -pie,$(LDFLAGS))
COMPILE:= $(CC) $(NOPIE_CFLAGS) $(CPPFLAGS) $(NOPIE_LDFLAGS)

all:
	# static version
	$(COMPILE) -std=c99 -static -o static basic.c

clean distclean:
	rm -f basic

check test:
