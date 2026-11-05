/* Opt-in cooperative timing probe. No network, vsock or shared guest storage.
 * A received symbol is work completed in a fixed middle-of-slot window, never
 * the sender's bit. Training/held-out decoding happens only in the host tool. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <time.h>
#include <termios.h>
#include <unistd.h>

#define MAX_SYMBOLS 4096
#define CPU_BATCH 4096
#define CACHE_BYTES (32u * 1024u * 1024u)
#define MEMORY_BYTES (128u * 1024u * 1024u)
static volatile uint64_t accumulator = 1;
static volatile uint64_t *memory;
static size_t words;
static size_t cursor;

static uint64_t now_ms(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_REALTIME, &t)) { perror("clock"); exit(2); }
    return (uint64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static void wait_until(uint64_t ms) {
    struct timespec t = {(time_t)(ms / 1000), (long)(ms % 1000) * 1000000};
    int result;
    do { result = clock_nanosleep(CLOCK_REALTIME, TIMER_ABSTIME, &t, NULL); } while (result == EINTR);
    if (result) { fprintf(stderr,"sleep: %s\n",strerror(result)); exit(2); }
}
static void batch(const char *channel) {
    uint64_t x = accumulator;
    if (!strcmp(channel,"cpu")) {
        for (size_t i=0; i<CPU_BATCH; ++i) x = x * 6364136223846793005ULL + 1;
    } else if (!strcmp(channel,"cache")) {
        /* Dependent pseudorandom cache-line accesses defeat streaming prefetch. */
        for (size_t i=0; i<CPU_BATCH; ++i) {
            cursor = (size_t)memory[cursor];
            x ^= cursor;
        }
    } else {
        for (size_t i=0; i<CPU_BATCH; ++i) {
            memory[cursor] = memory[cursor] + 1;
            cursor = (cursor + 8) % words;
        }
    }
    accumulator = x;
}
int main(int argc, char **argv) {
    int init = getpid() == 1;
    char cmdline[4096], arguments[4096];
    char *fields[5];
    if (init) {
        int console = open("/dev/console",O_RDWR);
        if (console >= 0) { dup2(console,0); dup2(console,1); dup2(console,2); }
        if (mount("proc","/proc","proc",0,NULL)) { perror("mount proc"); return 2; }
        FILE *f=fopen("/proc/cmdline","r");
        if (!f || !fgets(cmdline,sizeof(cmdline),f)) return 2;
        fclose(f);
        char *p=strstr(cmdline,"kiln_probe=");
        if (!p || sscanf(p+11,"%4095s",arguments)!=1) return 2;
        char *save=NULL;
        for (int i=0;i<5;++i) { fields[i]=strtok_r(i?NULL:arguments,",",&save); if (!fields[i]) return 2; }
    } else {
        if (argc != 6) { fprintf(stderr,"usage: probe sender|receiver cache|cpu|memory START_MS SYMBOL_MS BITS|COUNT\n"); return 2; }
        for (int i=0;i<5;++i) fields[i]=argv[i+1];
    }
    int sender=!strcmp(fields[0],"sender");
    if ((!sender && strcmp(fields[0],"receiver")) || (strcmp(fields[1],"cpu") && strcmp(fields[1],"cache") && strcmp(fields[1],"memory"))) return 2;
    uint64_t start=strtoull(fields[2],NULL,10), slot=strtoull(fields[3],NULL,10);
    size_t count=sender?strlen(fields[4]):strtoul(fields[4],NULL,10);
    if (slot<20 || slot>10000 || count<4 || count>MAX_SYMBOLS || start<now_ms()+100) { fprintf(stderr,"invalid or late probe schedule\n"); return 2; }
    if (sender && strspn(fields[4],"01")!=count) return 2;
    words=(!strcmp(fields[1],"memory")?MEMORY_BYTES:CACHE_BYTES)/sizeof(uint64_t);
    memory=calloc(words,sizeof(uint64_t));
    if (!memory) return 2;
    /* Multiplication by an odd value permutes cache line indexes. Chaining the
     * permutation in sequential traversal gives one cycle spanning the buffer. */
    size_t lines=words/8;
    for (size_t i=0;i<lines;++i) memory[((i*2654435761u)&(lines-1))*8]=(((i+1)*2654435761u)&(lines-1))*8;
    uint64_t samples[MAX_SYMBOLS];
    for (size_t i=0;i<count;++i) {
        uint64_t begin=start+i*slot, end=begin+slot;
        wait_until(begin);
        uint64_t work=0;
        if (sender) {
            if (fields[4][i]=='1') while (now_ms()<end) batch(fields[1]);
            else wait_until(end);
        } else {
            wait_until(begin+slot/5);
            while (now_ms()<end-slot/5) { batch(fields[1]); ++work; }
            wait_until(end);
        }
        samples[i] = work;
    }
    char report[MAX_SYMBOLS * 24 + 256];
    size_t used=(size_t)snprintf(report,sizeof(report),"KILN_PROBE {\"role\":\"%s\",\"channel\":\"%s\",\"start_ms\":%llu,\"symbol_ms\":%llu,\"samples\":[",fields[0],fields[1],(unsigned long long)start,(unsigned long long)slot);
    for (size_t i=0;i<count;++i) used+=(size_t)snprintf(report+used,sizeof(report)-used,"%s%llu",i?",":"",(unsigned long long)samples[i]);
    used+=(size_t)snprintf(report+used,sizeof(report)-used,"]}\n");
    size_t emitted=0;
    while (emitted<used) {
        ssize_t result=write(STDOUT_FILENO,report+emitted,used-emitted);
        if (result<0 && errno==EINTR) continue;
        if (result<=0) return 2;
        emitted+=(size_t)result;
    }
    if (init) tcdrain(STDOUT_FILENO);
    free((void *)memory);
    if (init) reboot(RB_AUTOBOOT);
    return 0;
}
