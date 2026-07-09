import os

def precise_refactor():
    file_path = 'platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt'
    with open(file_path, 'r') as f:
        content = f.read()

    # 1. Remove DeskdropJni (from "object DeskdropJni {" to its closing brace)
    # DeskdropJni is massive, let's find the exact string boundaries.
    start_str = "object DeskdropJni {"
    end_str = "    @JvmStatic external fun notifySleepState(handle: Long, isAsleep: Boolean): Int\n}"
    
    start_idx = content.find(start_str)
    end_idx = content.find(end_str) + len(end_str)
    
    if start_idx != -1 and end_idx != -1:
        content = content[:start_idx] + content[end_idx:]

    # 2. Remove ActivityFeed model
    start_str = "enum class ActivityKind {"
    end_str = "    val isApplicable: Boolean get() = kind == ActivityKind.CLIPBOARD_TEXT && !appliedLocally\n}"
    start_idx = content.find(start_str)
    end_idx = content.find(end_str) + len(end_str)
    if start_idx != -1 and end_idx != -1:
        content = content[:start_idx] + content[end_idx:]

    # 3. Remove TransferState/Progress
    start_str = "enum class TransferState {"
    end_str = "    val isOutbound: Boolean = false\n)"
    start_idx = content.find(start_str)
    end_idx = content.find(end_str) + len(end_str)
    if start_idx != -1 and end_idx != -1:
        content = content[:start_idx] + content[end_idx:]

    # 4. Remove feed from companion
    chunk_feed = """        // Global activity feed — readable by UI without binding to the service
        @JvmField val activityFeed = ArrayDeque<ActivityEntry>()
        @JvmField val feedLock     = Any()
        @JvmField val pendingOutboundTransferIds = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()

        fun addToFeed(entry: ActivityEntry) {
            synchronized(feedLock) {
                activityFeed.addFirst(entry)
                while (activityFeed.size > ACTIVITY_FEED_MAX) activityFeed.removeLast()
            }
        }
        
        fun removeFromFeed(id: Long) {
            synchronized(feedLock) {
                activityFeed.removeAll { it.id == id }
            }
        }

        fun getFeedSnapshot(): List<ActivityEntry> = synchronized(feedLock) {
            activityFeed.toList()
        }"""
    content = content.replace(chunk_feed, "")

    # 5. Remove transfers from companion
    chunk_flows = """        // Flow to expose active transfers to UI
        val activeTransfersFlow = kotlinx.coroutines.flow.MutableStateFlow<List<TransferProgress>>(emptyList())"""
    content = content.replace(chunk_flows, "")

    # 6. Remove activeTransfers map and publishActiveTransfers
    chunk_publish = """    private val activeTransfers = mutableMapOf<String, TransferProgress>()

    private fun publishActiveTransfers() {
        activeTransfersFlow.value = activeTransfers.values.toList()
    }"""
    content = content.replace(chunk_publish, "")

    # 7. Re-wire references
    content = content.replace("addToFeed(", "ActivityFeedManager.addToFeed(")
    content = content.replace("getFeedSnapshot()", "ActivityFeedManager.getFeedSnapshot()")
    content = content.replace("removeFromFeed(", "ActivityFeedManager.removeFromFeed(")
    content = content.replace("pendingOutboundTransferIds", "TransferManager.pendingOutboundTransferIds")
    content = content.replace("activeTransfers[", "TransferManager.activeTransfers[")
    content = content.replace("activeTransfers.", "TransferManager.activeTransfers.")
    content = content.replace("publishActiveTransfers()", "TransferManager.publishActiveTransfers()")
    content = content.replace("activeTransfersFlow", "TransferManager.activeTransfersFlow")
    content = content.replace("feedLock", "ActivityFeedManager.feedLock")
    content = content.replace("activityFeed", "ActivityFeedManager.activityFeed")
    
    # fix potential double replacement
    content = content.replace("ActivityFeedManager.ActivityFeedManager.", "ActivityFeedManager.")

    with open(file_path, 'w') as f:
        f.write(content)

precise_refactor()
