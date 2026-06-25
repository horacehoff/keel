// WORK IN PROGRESS!!

#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#include <windows.h>
// From https://stackoverflow.com/a/63114231
EXPORT int get_time(void) {
        SYSTEMTIME unix_epoch;
        unix_epoch.wYear = 1970;
        unix_epoch.wMonth = 1;
        unix_epoch.wDay = 1;
        unix_epoch.wDayOfWeek = 4;
        unix_epoch.wHour = 0;
        unix_epoch.wMilliseconds = 0;
        unix_epoch.wMinute = 0;
        unix_epoch.wSecond = 0;

        FILETIME curr_time_as_filetime;
        GetSystemTimeAsFileTime(&curr_time_as_filetime);

        FILETIME unix_epoch_as_filetime;
        SystemTimeToFileTime(&unix_epoch, &unix_epoch_as_filetime);

        ULARGE_INTEGER curr_time_as_uint64;
        ULARGE_INTEGER unix_epoch_as_uint64;

        curr_time_as_uint64.HighPart = curr_time_as_filetime.dwHighDateTime;
        curr_time_as_uint64.LowPart = curr_time_as_filetime.dwLowDateTime;

        unix_epoch_as_uint64.HighPart = unix_epoch_as_filetime.dwHighDateTime;
        unix_epoch_as_uint64.LowPart = unix_epoch_as_filetime.dwLowDateTime;

        ULARGE_INTEGER seconds_since_1970;
        seconds_since_1970.QuadPart = (curr_time_as_uint64.QuadPart - unix_epoch_as_uint64.QuadPart) / 10000000;

        return (int) seconds_since_1970.QuadPart;
}
#else
#include <time.h>
#define EXPORT __attribute__((visibility("default")))
EXPORT int get_time(void) {
    return (int) time(NULL);
}
#endif