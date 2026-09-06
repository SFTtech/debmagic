all: hello hello-static

hello: hello.c
	gcc hello.c -o hello

hello-static: hello.c
	gcc -static hello.c -o hello-static

clean:
	rm -f hello hello-static

distclean: clean
