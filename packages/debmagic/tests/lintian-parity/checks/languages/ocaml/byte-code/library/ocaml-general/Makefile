# A simple makefile to build a *.o file and a *.a file to install to test
# the OCaml checks for dangling *.cmx files.
all: symbol.a symbol-dev.a libbaz.a

%.a: %.o
	ar rv $@ $<
	# Not entirely sure this is acceptable for a normal OCaml
	# library, but for this test the library does not need
	# to work and it avoids a warning about unneeded sections.
	strip --remove-section=.comment --remove-section=.note \
	  --strip-debug $@

symbol-dev.o: symbol.c
	cc -c $< -o $@

libbaz.a: symbol-dev.o
	ar r $@ $<
