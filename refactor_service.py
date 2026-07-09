import os
import re

def refactor_service():
    service_file = 'platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt'
    with open(service_file, 'r') as f:
        content = f.read()

    # Remove ActivityFeed model (ActivityKind and ActivityEntry)
    content = re.sub(r'// ── Activity feed model ───────────────────────────────────────────────────────\s*enum class ActivityKind \{.*?\}\s*data class ActivityEntry\(.*?\}\s*', '', content, flags=re.DOTALL)
    
    # Remove TransferState and TransferProgress
    content = re.sub(r'enum class TransferState \{.*?\}\s*data class TransferProgress\(.*?\}\s*', '', content, flags=re.DOTALL)

    # Remove ActivityFeed methods from Companion
    content = re.sub(r'        // Global activity feed — readable by UI without binding to the service\s*@JvmField val activityFeed = ArrayDeque<ActivityEntry>\(\)\s*@JvmField val feedLock     = Any\(\)\s*@JvmField val pendingOutboundTransferIds = java\.util\.concurrent\.ConcurrentHashMap\.newKeySet<String>\(\)\s*fun addToFeed\(entry: ActivityEntry\) \{.*?\s*\}\s*fun removeFromFeed\(id: Long\) \{.*?\s*\}\s*fun getFeedSnapshot\(\): List<ActivityEntry> = synchronized\(feedLock\) \{.*?\s*\}\s*', '', content, flags=re.DOTALL)
    
    # Remove TransferManager flows from Companion
    content = re.sub(r'        // Flow to expose active transfers to UI\s*val activeTransfersFlow = kotlinx\.coroutines\.flow\.MutableStateFlow<List<TransferProgress>>\(emptyList\(\)\)\s*', '', content, flags=re.DOTALL)
    
    # Remove activeTransfers map and publish method
    content = re.sub(r'    private val activeTransfers = mutableMapOf<String, TransferProgress>\(\)\s*private fun publishActiveTransfers\(\) \{\s*activeTransfersFlow\.value = activeTransfers\.values\.toList\(\)\s*\}\s*', '', content, flags=re.DOTALL)
    
    # In DeskdropService.kt, replace addToFeed with ActivityFeedManager.addToFeed
    content = content.replace("addToFeed(", "ActivityFeedManager.addToFeed(")
    content = content.replace("getFeedSnapshot()", "ActivityFeedManager.getFeedSnapshot()")
    content = content.replace("removeFromFeed(", "ActivityFeedManager.removeFromFeed(")
    
    content = content.replace("pendingOutboundTransferIds", "TransferManager.pendingOutboundTransferIds")
    content = content.replace("activeTransfers[", "TransferManager.activeTransfers[")
    content = content.replace("activeTransfers.", "TransferManager.activeTransfers.")
    content = content.replace("publishActiveTransfers()", "TransferManager.publishActiveTransfers()")
    content = content.replace("activeTransfersFlow", "TransferManager.activeTransfersFlow")

    with open(service_file, 'w') as f:
        f.write(content)

def update_ui_references():
    search_dir = 'platforms/android/app/src/main/java/com/deskdrop'
    for root, _, files in os.walk(search_dir):
        for file in files:
            if file.endswith('.kt') and file != 'DeskdropService.kt':
                path = os.path.join(root, file)
                with open(path, 'r') as f:
                    content = f.read()
                
                original = content
                
                content = content.replace("DeskdropService.getFeedSnapshot()", "ActivityFeedManager.getFeedSnapshot()")
                content = content.replace("DeskdropService.removeFromFeed(", "ActivityFeedManager.removeFromFeed(")
                content = content.replace("DeskdropService.addToFeed(", "ActivityFeedManager.addToFeed(")
                content = content.replace("DeskdropService.activeTransfersFlow", "TransferManager.activeTransfersFlow")
                
                if content != original:
                    with open(path, 'w') as f:
                        f.write(content)
                        print(f"Updated references in {path}")

refactor_service()
update_ui_references()
