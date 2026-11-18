#import "virtual_display.h"
#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <objc/runtime.h>

@class CGVirtualDisplayDescriptor;
@class CGVirtualDisplaySettings;
@class CGVirtualDisplayMode;
@class CGVirtualDisplay;

@interface CGVirtualDisplayMode : NSObject
- (instancetype)initWithWidth:(NSUInteger)width height:(NSUInteger)height refreshRate:(CGFloat)refreshRate;
@end

@interface CGVirtualDisplaySettings : NSObject
@property(retain, nonatomic) NSArray<CGVirtualDisplayMode *> *modes;
@property(nonatomic) unsigned int hiDPI;
- (instancetype)init;
@end

@interface CGVirtualDisplayDescriptor : NSObject
@property(retain, nonatomic) dispatch_queue_t queue;
@property(retain, nonatomic) NSString *name;
@property(nonatomic) unsigned int maxPixelsHigh;
@property(nonatomic) unsigned int maxPixelsWide;
@property(nonatomic) CGSize sizeInMillimeters;
@property(nonatomic) unsigned int serialNum;
@property(nonatomic) unsigned int productID;
@property(nonatomic) unsigned int vendorID;
@property(copy, nonatomic) void (^terminationHandler)(id _Nullable, CGVirtualDisplay*);
- (instancetype)init;
@end

@interface CGVirtualDisplay : NSObject
@property(readonly, nonatomic) CGDirectDisplayID displayID;
- (instancetype)initWithDescriptor:(CGVirtualDisplayDescriptor *)descriptor;
- (BOOL)applySettings:(CGVirtualDisplaySettings *)settings;
@end

static NSMutableDictionary<NSNumber *, CGVirtualDisplay *> *s_active_displays = nil;
static NSLock *s_lock = nil;

static void ensure_initialized(void) {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        s_active_displays = [[NSMutableDictionary alloc] init];
        s_lock = [[NSLock alloc] init];
    });
}

bool sm_virtual_display_is_supported(void) {
    Class displayClass = NSClassFromString(@"CGVirtualDisplay");
    Class descClass = NSClassFromString(@"CGVirtualDisplayDescriptor");
    Class settingsClass = NSClassFromString(@"CGVirtualDisplaySettings");
    Class modeClass = NSClassFromString(@"CGVirtualDisplayMode");
    return (displayClass != nil && descClass != nil && settingsClass != nil && modeClass != nil);
}

uint32_t sm_virtual_display_create(const char *name, uint32_t width, uint32_t height, double refresh_rate, uint32_t hi_dpi) {
    if (!sm_virtual_display_is_supported()) {
        NSLog(@"[virtual_display] CGVirtualDisplay is not supported on this system");
        return 0;
    }

    if (width == 0 || height == 0) {
        width = 1920;
        height = 1080;
    }
    if (refresh_rate <= 0.0) {
        refresh_rate = 60.0;
    }

    ensure_initialized();

    @autoreleasepool {
        Class descClass = NSClassFromString(@"CGVirtualDisplayDescriptor");
        Class displayClass = NSClassFromString(@"CGVirtualDisplay");
        Class settingsClass = NSClassFromString(@"CGVirtualDisplaySettings");
        Class modeClass = NSClassFromString(@"CGVirtualDisplayMode");

        CGVirtualDisplayDescriptor *desc = [[descClass alloc] init];
        NSString *displayName = name && strlen(name) > 0
            ? [NSString stringWithUTF8String:name]
            : @"ScreenMirror Extended Display";
        desc.name = displayName;

        uint32_t pixelWidth = (hi_dpi != 0) ? (width * 2) : width;
        uint32_t pixelHeight = (hi_dpi != 0) ? (height * 2) : height;

        desc.maxPixelsWide = pixelWidth;
        desc.maxPixelsHigh = pixelHeight;
        desc.sizeInMillimeters = CGSizeMake((CGFloat)width * 0.25, (CGFloat)height * 0.25);
        desc.productID = 0x534D; // 'SM'
        desc.vendorID = 0x05AC;  // Apple-compatible virtual vendor ID
        desc.serialNum = (unsigned int)(arc4random_uniform(90000) + 10000);
        desc.queue = dispatch_get_main_queue();

        __weak CGVirtualDisplayDescriptor *weakDesc = desc;
        desc.terminationHandler = ^(id _Nullable reason, CGVirtualDisplay *terminatedDisplay) {
            #pragma unused(weakDesc)
            if (terminatedDisplay) {
                uint32_t tid = terminatedDisplay.displayID;
                NSLog(@"[virtual_display] Virtual display %u terminated by system: %@", tid, reason);
                [s_lock lock];
                [s_active_displays removeObjectForKey:@(tid)];
                [s_lock unlock];
            }
        };

        CGVirtualDisplay *display = [[displayClass alloc] initWithDescriptor:desc];
        if (!display) {
            NSLog(@"[virtual_display] Failed to instantiate CGVirtualDisplay");
            return 0;
        }

        CGVirtualDisplaySettings *settings = [[settingsClass alloc] init];
        CGVirtualDisplayMode *mode = [[modeClass alloc] initWithWidth:pixelWidth
                                                               height:pixelHeight
                                                          refreshRate:(CGFloat)refresh_rate];
        settings.modes = @[mode];
        settings.hiDPI = (hi_dpi != 0) ? 1 : 0;

        BOOL applied = [display applySettings:settings];
        if (!applied) {
            NSLog(@"[virtual_display] Failed to apply settings to CGVirtualDisplay");
            return 0;
        }

        uint32_t displayID = display.displayID;
        if (displayID == 0) {
            NSLog(@"[virtual_display] CGVirtualDisplay returned displayID 0");
            return 0;
        }

        [s_lock lock];
        s_active_displays[@(displayID)] = display;
        [s_lock unlock];

        NSLog(@"[virtual_display] Created virtual display '%@' with ID: %u (%ux%u @ %.1fHz, hiDPI: %u)",
              displayName, displayID, width, height, refresh_rate, hi_dpi);

        return displayID;
    }
}

bool sm_virtual_display_destroy(uint32_t display_id) {
    ensure_initialized();

    [s_lock lock];
    CGVirtualDisplay *display = s_active_displays[@(display_id)];
    if (display) {
        [s_active_displays removeObjectForKey:@(display_id)];
        [s_lock unlock];
        NSLog(@"[virtual_display] Destroyed virtual display ID: %u", display_id);
        return true;
    }
    [s_lock unlock];
    return false;
}

void sm_virtual_display_destroy_all(void) {
    ensure_initialized();

    [s_lock lock];
    NSUInteger count = s_active_displays.count;
    [s_active_displays removeAllObjects];
    [s_lock unlock];

    if (count > 0) {
        NSLog(@"[virtual_display] Destroyed all %lu virtual display(s)", (unsigned long)count);
    }
}

uint32_t sm_virtual_display_get_active_ids(uint32_t *out_ids, uint32_t max_count) {
    ensure_initialized();

    [s_lock lock];
    NSArray<NSNumber *> *keys = [s_active_displays allKeys];
    uint32_t count = (uint32_t)keys.count;
    if (out_ids && max_count > 0) {
        uint32_t to_copy = (count < max_count) ? count : max_count;
        for (uint32_t i = 0; i < to_copy; i++) {
            out_ids[i] = [keys[i] unsignedIntValue];
        }
    }
    [s_lock unlock];
    return count;
}
