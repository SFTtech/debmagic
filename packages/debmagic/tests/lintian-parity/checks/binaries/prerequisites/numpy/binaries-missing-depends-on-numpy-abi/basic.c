#include <Python.h>
#include <numpy/arrayobject.h>
#include <stdio.h>
#include <string.h>

#define NPY_NO_DEPRECATED_API NPY_1_7_API_VERSION

static void
hardening_trigger(char *p, int i, void (*f)(char *))
{
    char test[10];
    memcpy(test, p, i);
    f(test);
    printf("%s", test);
}

void * do_import_array(void)
{
    import_array();
    hardening_trigger(NULL, 0, NULL);
    return NULL;
}
