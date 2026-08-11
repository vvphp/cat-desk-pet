#ifdef __APPLE__

#include <errno.h>
#include <inttypes.h>
#include <libproc.h>
#include <mach/mach_time.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: process-cpu-time PID\n");
        return 2;
    }

    char *end = NULL;
    errno = 0;
    long parsed = strtol(argv[1], &end, 10);
    if (errno != 0 || end == argv[1] || *end != '\0' || parsed <= 0) {
        fprintf(stderr, "invalid PID: %s\n", argv[1]);
        return 2;
    }

    struct rusage_info_v4 usage = {0};
    if (proc_pid_rusage((int)parsed, RUSAGE_INFO_V4, (rusage_info_t *)&usage) != 0) {
        perror("proc_pid_rusage");
        return 1;
    }

    mach_timebase_info_data_t timebase = {0};
    if (mach_timebase_info(&timebase) != KERN_SUCCESS || timebase.denom == 0) {
        fprintf(stderr, "mach_timebase_info failed\n");
        return 1;
    }

    uint64_t absolute_ticks = usage.ri_user_time + usage.ri_system_time;
    long double nanoseconds =
        (long double)absolute_ticks * timebase.numer / timebase.denom;
    printf("%.9Lf\n", nanoseconds / 1000000000.0L);
    return 0;
}

#else
#error "process-cpu-time is a macOS-only benchmark helper"
#endif
