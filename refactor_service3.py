import os
import re

def fix():
    file_path = 'platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt'
    with open(file_path, 'r') as f:
        content = f.read()
        
    # Remove DeskdropJni
    # Use exact line boundaries or regex since it's a big block.
    # It starts with "object DeskdropJni {" and ends with "}" right before "Activity feed model"
    content = re.sub(r'// ── JNI Bridge ────────────────────────────────────────────────────────────────.*?\}\n\n// ── Activity feed model', '// ── Activity feed model', content, flags=re.DOTALL)
    
    # Replace feedLock and activityFeed
    content = content.replace("feedLock", "ActivityFeedManager.feedLock")
    content = content.replace("activityFeed", "ActivityFeedManager.activityFeed")
    
    # But wait! I previously replaced "ActivityFeedManager.activityFeed" which means if it's already there it will become ActivityFeedManager.ActivityFeedManager.activityFeed.
    # Let's fix that.
    content = content.replace("ActivityFeedManager.ActivityFeedManager.", "ActivityFeedManager.")
    
    # We might also need to replace it in MainActivity again if not done? No, I ran update_ui_references.
    with open(file_path, 'w') as f:
        f.write(content)

fix()
