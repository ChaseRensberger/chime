#import <ApplicationServices/ApplicationServices.h>
#import <CoreServices/CoreServices.h>
#import <AVFoundation/AVFoundation.h>

int chime_microphone_status(void) {
    return (int)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
}

void chime_request_microphone(void) {
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted) {}];
}

bool chime_accessibility_ready(void) {
    return AXIsProcessTrusted();
}

int chime_automation_status(void) {
    const char target[] = "com.apple.systemevents";
    AEAddressDesc desc = {0};
    OSStatus status = AECreateDesc(typeApplicationBundleID, target, sizeof(target) - 1, &desc);
    if (status != noErr) return status;
    status = AEDeterminePermissionToAutomateTarget(&desc, typeWildCard, typeWildCard, false);
    AEDisposeDesc(&desc);
    return status;
}
