int get(int a, int (*f)(int)) {
	return f(a);
}

extern int foo(int);

