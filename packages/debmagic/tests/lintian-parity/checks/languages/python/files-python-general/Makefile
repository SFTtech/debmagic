all: lib.c
	echo 'Hallo World' > code.pyc
	echo 'Hallo World' > code.pyo
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -fPIC -shared -o python-module.so lib.c

distclean:
	rm -f code.pyc code.pyo python-module.so
