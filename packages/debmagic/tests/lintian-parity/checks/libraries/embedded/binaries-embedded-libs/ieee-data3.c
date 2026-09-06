#include <stdio.h>

struct ieee_data {
  char a1;
  char a2;
  char a3;
  const char * name;
};

static const struct ieee_data ieee_data_array[]
    = { 0x00, 0x00, 0x56, "DR. B. STRUCK"};

int
main(void)
{
    printf("%s\n", ieee_data_array[0].name);
}
