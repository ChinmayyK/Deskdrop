import SwiftUI

class DynamicHeaderManager: ObservableObject {
    static let shared = DynamicHeaderManager()
    
    @Published var dailyTagline: String = DeskdropTaglines.general[0]
    
    private let defaults = UserDefaults.standard
    private let lastUpdatedKey = "deskdrop_hero_last_updated"
    private let lastIndicesKey = "deskdrop_hero_last_indices"
    
    private init() {
        refreshDailyTaglineIfNeeded()
    }
    
    func refreshDailyTaglineIfNeeded() {
        let calendar = Calendar.current
        let now = Date()
        
        let lastUpdated = defaults.object(forKey: lastUpdatedKey) as? Date ?? Date.distantPast
        
        // If it's the same calendar day, keep the current one (if we have one). 
        // But since this is init, we need to load the *currently selected* one from history if available.
        var history = defaults.array(forKey: lastIndicesKey) as? [Int] ?? []
        
        if calendar.isDate(now, inSameDayAs: lastUpdated) {
            if let lastIndex = history.last, lastIndex >= 0 && lastIndex < DeskdropTaglines.general.count {
                dailyTagline = DeskdropTaglines.general[lastIndex]
                return
            }
        }
        
        // Pick a new tagline
        let poolCount = DeskdropTaglines.general.count
        var availableIndices = Set(0..<poolCount)
        
        // Remove the last 15 indices to prevent repetition
        for idx in history.suffix(15) {
            availableIndices.remove(idx)
        }
        
        // If we somehow ran out of indices (e.g. pool is smaller than 15), reset available
        if availableIndices.isEmpty {
            availableIndices = Set(0..<poolCount)
            if let lastIndex = history.last {
                availableIndices.remove(lastIndex) // Just avoid immediate repeat
            }
        }
        
        // Pick random
        let newIndex = availableIndices.randomElement() ?? 0
        
        // Save state
        history.append(newIndex)
        // Keep history bounded to 20
        if history.count > 20 {
            history.removeFirst(history.count - 20)
        }
        
        defaults.set(history, forKey: lastIndicesKey)
        defaults.set(now, forKey: lastUpdatedKey)
        
        dailyTagline = DeskdropTaglines.general[newIndex]
    }
}
