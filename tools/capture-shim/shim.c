/* Logging stand-in for the official app's hiddriver_N.dll.
 *
 * Every SetFeature / GetFeature / Set_VIDPID call is appended to capture.log
 * (next to Mouse.exe) and then passed to the real driver, renamed hiddriver_No.dll.
 * The official Mouse.exe only ever calls these three plus Open_FeatureDevice,
 * so this log is a complete record of what it sends to the mouse.
 *
 * Build one copy per driver: cl /DDRV=1 ... and cl /DDRV=2 ... (see build.bat).
 */
#include <windows.h>
#include <stdio.h>

#define STR2(x) #x
#define STR(x) STR2(x)
#define REAL "hiddriver_" STR(DRV) "o.dll"

typedef BOOL(WINAPI *SF)(BYTE *, int);
typedef BOOL(WINAPI *VP)(int, int);
static HMODULE real;
static FILE *lg;

static FARPROC f(const char *n) {
    if (!real) real = LoadLibraryA(REAL);
    return GetProcAddress(real, n);
}

static void dump(const char *tag, BYTE *b, int len, BOOL ok) {
    SYSTEMTIME t;
    GetLocalTime(&t);
    if (!lg) lg = fopen("capture.log", "a");
    fprintf(lg, "%02d:%02d:%02d.%03d drv%d %s len=%d ok=%d :", t.wHour, t.wMinute, t.wSecond, t.wMilliseconds, DRV, tag, len, ok);
    int n = len;
    while (n > 1 && b[n - 1] == 0) n--; /* trim trailing zero padding */
    for (int i = 0; i < n; i++) fprintf(lg, " %02x", b[i]);
    fprintf(lg, "\n");
    fflush(lg);
}

/* everything else goes straight to the real driver */
#define FWD(name) comment(linker, "/export:" #name "=hiddriver_" STR(DRV) "o." #name)
#pragma FWD(Open_FeatureDevice)
#pragma FWD(Close_FeatureDevice)
#pragma FWD(Open_DevMonitor)
#pragma FWD(Close_DevMonitor)
#pragma FWD(Hid_Read)
#pragma FWD(Hid_Write)

__declspec(dllexport) BOOL WINAPI SetFeature(BYTE *b, int len) { BOOL r = ((SF)f("SetFeature"))(b, len); dump("SET", b, len, r); return r; }
__declspec(dllexport) BOOL WINAPI GetFeature(BYTE *b, int len) { BOOL r = ((SF)f("GetFeature"))(b, len); dump("GET", b, len, r); return r; }
__declspec(dllexport) BOOL WINAPI Set_VIDPID(int v, int p) {
    BYTE x[8];
    *(int *)x = v;
    *(int *)(x + 4) = p;
    BOOL r = ((VP)f("Set_VIDPID"))(v, p);
    dump("VIDPID", x, 8, r);
    return r;
}
