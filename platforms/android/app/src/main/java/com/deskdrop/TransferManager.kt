package com.deskdrop

import kotlinx.coroutines.flow.MutableStateFlow

enum class TransferState {
    INCOMING, PROGRESS, PAUSED, CANCELED, FAILED, COMPLETED
}

data class TransferProgress(
    val id: String,
    val fileName: String,
    val percent: Int,
    val bytesReceived: Long,
    val totalBytes: Long = 0,
    val speedBps: Long,
    val etaSecs: Long,
    var isPaused: Boolean = false,
    val state: TransferState = TransferState.PROGRESS,
    val peerName: String = "",
    val isOutbound: Boolean = false
)

object TransferManager {
    // Flow to expose active transfers to UI
    val activeTransfersFlow = MutableStateFlow<List<TransferProgress>>(emptyList())
    
    val pendingOutboundTransferIds = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
    
    // Maintain state here instead of in DeskdropService directly (or service delegates to this)
    val activeTransfers = mutableMapOf<String, TransferProgress>()

    fun publishActiveTransfers() {
        activeTransfersFlow.value = activeTransfers.values.toList()
    }
}
