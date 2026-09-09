/* This program uses the obsolete function 'encrypt', which performs
   DES encryption.  */

#define _GNU_SOURCE 1
#include <unistd.h>
#include <string.h>
#include <stdio.h>

/* The prototype of 'encrypt' may already have been removed from
   unistd.h.  */
extern void encrypt(char block[64], int edflag);

/* It may already not be possible to link new programs that use
   'encrypt' without special magic.  */
#ifdef SYMVER
__asm__ (".symver encrypt, encrypt@" SYMVER);
#endif

int
main(void)
{
    char block[64];
    memset(block, 0, sizeof block);
    encrypt(block, 0);
    for (size_t i = 0; i < sizeof block; i++) {
        putchar(block[i] ? '1' : '0');
    }
    putchar('\n');
    return 0;
}
