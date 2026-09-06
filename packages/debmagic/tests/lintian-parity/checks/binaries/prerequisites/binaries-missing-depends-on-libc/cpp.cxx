#include <iostream>
#include <vector>

using namespace std;

template <class T> T
cxx_lib_interface(vector<T> &v)
{
    cout << ":-) hello world" << endl;
    return v[0];
}



