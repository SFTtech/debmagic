int do_something(int (*a)(char *)){
  char test[10];
  return a(test);
}
