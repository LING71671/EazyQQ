#include <windows.h>
#include <fstream>

int wmain() {
    // This fixture never loads the supplied DLL or starts QQ.
    std::ofstream("fixture-launched.txt") << GetCurrentProcessId();
    Sleep(60000);
    return 0;
}
