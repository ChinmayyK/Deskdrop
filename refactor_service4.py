import os

def refactor_line_by_line():
    file_path = 'platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt'
    with open(file_path, 'r') as f:
        lines = f.readlines()

    out_lines = []
    skip = False
    
    for i, line in enumerate(lines):
        # 1. Skip JNI bridge
        if line.startswith('// ── JNI Bridge'):
            skip = True
        
        # 2. Skip Activity Feed model
        if line.startswith('// ── Activity feed model'):
            skip = True
            
        # 3. Skip TransferState/Progress
        if line.startswith('enum class TransferState'):
            skip = True
            
        # 4. Skip activity feed in Companion object
        if line.strip() == '@JvmField val activityFeed = ArrayDeque<ActivityEntry>()':
            if out_lines and "Global activity feed" in out_lines[-1]:
                out_lines.pop() # remove previous line "// Global activity feed"
            skip = True
            
        # 5. Skip Transfer flow
        if line.strip() == 'val activeTransfersFlow = kotlinx.coroutines.flow.MutableStateFlow<List<TransferProgress>>(emptyList())':
            if out_lines and "Flow to expose active transfers to UI" in out_lines[-1]:
                out_lines.pop() # remove previous line "// Flow to expose active transfers"
            skip = True
            
        # 6. Skip activeTransfers private map and publish method
        if line.strip() == 'private val activeTransfers = mutableMapOf<String, TransferProgress>()':
            skip = True

        if not skip:
            # refactor inline references
            mod = line
            mod = mod.replace("addToFeed(", "ActivityFeedManager.addToFeed(")
            mod = mod.replace("getFeedSnapshot()", "ActivityFeedManager.getFeedSnapshot()")
            mod = mod.replace("removeFromFeed(", "ActivityFeedManager.removeFromFeed(")
            mod = mod.replace("pendingOutboundTransferIds", "TransferManager.pendingOutboundTransferIds")
            mod = mod.replace("activeTransfers[", "TransferManager.activeTransfers[")
            mod = mod.replace("activeTransfers.", "TransferManager.activeTransfers.")
            mod = mod.replace("publishActiveTransfers()", "TransferManager.publishActiveTransfers()")
            mod = mod.replace("activeTransfersFlow", "TransferManager.activeTransfersFlow")
            
            # replace feedLock and activityFeed directly, careful about context
            # We want to replace feedLock -> ActivityFeedManager.feedLock
            mod = mod.replace("feedLock", "ActivityFeedManager.feedLock")
            mod = mod.replace("activityFeed", "ActivityFeedManager.activityFeed")
            
            # Since we replaced ActivityFeedManager.addToFeed previously, if it contained activityFeed it might double up.
            mod = mod.replace("ActivityFeedManager.ActivityFeedManager.", "ActivityFeedManager.")
            
            out_lines.append(mod)

        # Stop skipping conditions
        if skip:
            if line.startswith('}'):
                if i > 0 and 'notifySleepState' in lines[i-1]:
                    skip = False
                    continue
                if i > 0 and 'val isApplicable: Boolean' in lines[i-1]:
                    skip = False
                    continue
                if i > 0 and '        }' in lines[i] and 'activityFeed.toList()' in lines[i-1]:
                    skip = False
                    continue
                if i > 0 and '    }' in lines[i] and 'activeTransfersFlow.value =' in lines[i-1]:
                    skip = False
                    continue

            # TransferProgress ends with ) instead of }
            if line.startswith(')'):
                if i > 0 and 'val isOutbound: Boolean = false' in lines[i-1]:
                    skip = False
                    continue

    with open(file_path, 'w') as f:
        f.writelines(out_lines)

refactor_line_by_line()
