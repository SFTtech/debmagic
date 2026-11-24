extern int get(int, int (*)(int));

int foo(int a) {
	int b = a;
	int bar(int a) {
		return a + b;
	}
	return get(a, bar);
}
