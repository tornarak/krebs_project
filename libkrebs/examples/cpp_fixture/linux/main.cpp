// CppComponent.cpp : Defines the entry point for the console application.
//

#include <iostream>
#include <random>
#include <string>
#include <vector>

#include <unistd.h>

using namespace std;

void inspect_string(string *str) {
  unsigned char *str_bin = (unsigned char *)str;
  printf("Size: %08lX\nString Length: %08lX\nBase Address: %08lX\nC String "
         "Address: %08lX\n",
         sizeof(string), str->length(), (size_t) & (*str),
         (size_t)str->c_str());

  cout << "[ ";
  for (unsigned int i = 0; i < sizeof(string); ++i) {
    printf("0x%02x, ", str_bin[i]);
  }
  cout << " ]\n\n";
}

int main() {
  printf("PID: %d\n", getpid());
  string short_str = "Hello, World!";
  inspect_string(&short_str);

  size_t str_addr;

  try {
    cout << "string address: ";

    cin >> hex >> str_addr;
    cout << ((str_addr == (size_t)&short_str) ? "correct" : "incorrect")
         << endl;

    string g;
    getline(cin, g);
  } catch (...) {
    cout << "ERR";
    string g;
    getline(cin, g);
  }
}
