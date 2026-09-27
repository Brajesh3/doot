package com.brajesh.doot.ui.chat

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.InsertDriveFile
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Done
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.NotificationsOff
import androidx.compose.material.icons.outlined.PriorityHigh
import androidx.compose.material.icons.outlined.QrCode
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import com.brajesh.doot.messenger.AttachmentItem
import com.brajesh.doot.messenger.MessageItem
import com.brajesh.doot.messenger.MessengerViewModel
import java.io.File

import com.brajesh.doot.ui.components.expressivePressScale


@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ChatScreen(
    viewModel: MessengerViewModel,
    onBack: () -> Unit,
    modifier: Modifier = Modifier
) {
    val state by viewModel.uiState.collectAsState()
    val activePeer = state.activePeer
    val context = LocalContext.current
    var inputText by remember { mutableStateOf("") }
    var showFloatingToolbar by remember { mutableStateOf(false) }
    var showSendMenu by remember { mutableStateOf(false) }
    var selectedMessageId by remember { mutableStateOf<String?>(null) }
    val listState = rememberLazyListState()

    BackHandler {
        if (selectedMessageId != null) {
            selectedMessageId = null
        } else {
            viewModel.clearActivePeer()
            onBack()
        }
    }

    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri: Uri? ->
        uri?.let {
            val path = copyUriToCache(context, it)
            if (path != null) {
                viewModel.sendFile(path, isDirectory = false)
            } else {
                Toast.makeText(context, "Could not access selected file", Toast.LENGTH_SHORT).show()
            }
        }
    }

    LaunchedEffect(state.messages.size) {
        if (state.messages.isNotEmpty()) {
            listState.animateScrollToItem(state.messages.size - 1)
        }
    }

    val selectedMessage = state.messages.find { it.id == selectedMessageId }

    Scaffold(
        modifier = modifier.fillMaxSize(),
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            modifier = Modifier
                                .size(40.dp)
                                .clip(CircleShape)
                                .background(MaterialTheme.colorScheme.primaryContainer),
                            contentAlignment = Alignment.Center
                        ) {
                            val initial = activePeer?.nickname?.firstOrNull()?.uppercase() ?: "P"
                            Text(
                                text = initial,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onPrimaryContainer
                            )
                        }
                        Spacer(modifier = Modifier.width(12.dp))
                        Column {
                            Text(
                                text = activePeer?.nickname ?: "Chat",
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                            val connInfo = if (activePeer != null) state.connectionTypes[activePeer.peerKey] else null
                            if (connInfo != null) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.clickable {
                                        activePeer?.let { viewModel.pingPeer(it.peerKey) }
                                        Toast.makeText(context, "Measuring live P2P RTT...", Toast.LENGTH_SHORT).show()
                                    }
                                ) {
                                    val dotColor = if (connInfo.isDirect()) Color(0xFF1E8E3E) else Color(0xFFE37400)
                                    Box(
                                        modifier = Modifier
                                            .size(8.dp)
                                            .clip(CircleShape)
                                            .background(dotColor)
                                    )
                                    Spacer(modifier = Modifier.width(4.dp))
                                    Text(
                                        text = "${if (connInfo.isDirect()) "Direct UDP" else "DERP Relay"} • ${connInfo.rttMs}ms",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            } else {
                                Text(
                                    text = "Encrypted QUIC",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                    }
                },
                navigationIcon = {
                    val backInteraction = remember { MutableInteractionSource() }
                    IconButton(
                        onClick = {
                            if (selectedMessageId != null) {
                                selectedMessageId = null
                            } else {
                                viewModel.clearActivePeer()
                                onBack()
                            }
                        },
                        modifier = Modifier.expressivePressScale(backInteraction)
                    ) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                },
                actions = {
                    val pingInteraction = remember { MutableInteractionSource() }
                    IconButton(
                        onClick = {
                            activePeer?.let {
                                viewModel.pingPeer(it.peerKey)
                                Toast.makeText(context, "Pinged peer over QUIC", Toast.LENGTH_SHORT).show()
                            }
                        },
                        modifier = Modifier.expressivePressScale(pingInteraction)
                    ) {
                        Icon(Icons.Default.Refresh, contentDescription = "Ping peer")
                    }

                    val copyInteraction = remember { MutableInteractionSource() }
                    IconButton(
                        onClick = {
                            activePeer?.let {
                                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                val clip = ClipData.newPlainText("Peer Key", it.peerKey)
                                clipboard.setPrimaryClip(clip)
                                Toast.makeText(context, "Peer public key copied", Toast.LENGTH_SHORT).show()
                            }
                        },
                        modifier = Modifier.expressivePressScale(copyInteraction)
                    ) {
                        Icon(Icons.Outlined.ContentCopy, contentDescription = "Copy peer key")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface
                )
            )
        },
        bottomBar = {
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .imePadding()
                    .navigationBarsPadding(),
                color = MaterialTheme.colorScheme.surfaceContainer,
                tonalElevation = 6.dp
            ) {
                Column(modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp)) {
                    // Contextual Multi-Select / Quick Options via ButtonGroup
                    AnimatedVisibility(
                        visible = selectedMessage != null,
                        enter = fadeIn(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()) +
                                expandVertically(animationSpec = MaterialTheme.motionScheme.slowSpatialSpec()),
                        exit = fadeOut(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()) +
                                shrinkVertically(animationSpec = MaterialTheme.motionScheme.slowSpatialSpec())
                    ) {
                        Card(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 4.dp, vertical = 4.dp),
                            shape = RoundedCornerShape(16.dp),
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh
                            )
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 8.dp, vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Text(
                                    text = "Selected",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.primary,
                                    fontWeight = FontWeight.SemiBold
                                )

                                ButtonGroup(
                                    overflowIndicator = {
                                        IconButton(onClick = { selectedMessageId = null }) {
                                            Icon(Icons.Default.Close, contentDescription = "Deselect", modifier = Modifier.size(18.dp))
                                        }
                                    }
                                ) {
                                    clickableItem(
                                        onClick = {
                                            selectedMessage?.let { msg ->
                                                val text = msg.attachment?.fileName ?: msg.content
                                                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                                clipboard.setPrimaryClip(ClipData.newPlainText("Message Text", text))
                                                Toast.makeText(context, "Copied to clipboard", Toast.LENGTH_SHORT).show()
                                            }
                                            selectedMessageId = null
                                        },
                                        label = "Copy",
                                        icon = {
                                            Icon(
                                                imageVector = Icons.Outlined.ContentCopy,
                                                contentDescription = "Copy",
                                                modifier = Modifier.size(16.dp)
                                            )
                                        }
                                    )
                                    clickableItem(
                                        onClick = {
                                            selectedMessage?.let { msg ->
                                                val details = "Status: ${msg.status}\nTime: ${msg.timestamp}\nEncrypted via QUIC"
                                                Toast.makeText(context, details, Toast.LENGTH_SHORT).show()
                                            }
                                            selectedMessageId = null
                                        },
                                        label = "Details",
                                        icon = {
                                            Icon(
                                                imageVector = Icons.Outlined.Info,
                                                contentDescription = "Details",
                                                modifier = Modifier.size(16.dp)
                                            )
                                        }
                                    )
                                    clickableItem(
                                        onClick = { selectedMessageId = null },
                                        label = "Done",
                                        icon = {
                                            Icon(
                                                imageVector = Icons.Default.Close,
                                                contentDescription = "Close",
                                                modifier = Modifier.size(16.dp)
                                            )
                                        }
                                    )
                                }
                            }
                        }
                    }

                    // Peer typing state
                    AnimatedVisibility(
                        visible = state.isPeerTyping,
                        enter = fadeIn(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()) +
                                expandVertically(animationSpec = MaterialTheme.motionScheme.slowSpatialSpec()),
                        exit = fadeOut(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()) +
                                shrinkVertically(animationSpec = MaterialTheme.motionScheme.slowSpatialSpec())
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 12.dp, vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            LoadingIndicator(
                                modifier = Modifier.size(14.dp),
                                color = MaterialTheme.colorScheme.primary
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = "Peer is typing over QUIC...",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.primary
                            )
                        }
                    }

                    // Quick Actions Floating Toolbar (replaces static bottom sheets)
                    AnimatedVisibility(
                        visible = showFloatingToolbar,
                        enter = fadeIn(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()) +
                                expandVertically(animationSpec = MaterialTheme.motionScheme.slowSpatialSpec()),
                        exit = fadeOut(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()) +
                                shrinkVertically(animationSpec = MaterialTheme.motionScheme.slowSpatialSpec())
                    ) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 4.dp),
                            contentAlignment = Alignment.Center
                        ) {
                            HorizontalFloatingToolbar(
                                expanded = true,
                                shape = RoundedCornerShape(20.dp),
                                colors = FloatingToolbarDefaults.standardFloatingToolbarColors(
                                    toolbarContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh
                                )
                            ) {
                                val fileInt = remember { MutableInteractionSource() }
                                IconButton(
                                    onClick = {
                                        showFloatingToolbar = false
                                        filePickerLauncher.launch("*/*")
                                    },
                                    modifier = Modifier.expressivePressScale(fileInt)
                                ) {
                                    Icon(
                                        imageVector = Icons.AutoMirrored.Filled.InsertDriveFile,
                                        contentDescription = "File",
                                        tint = MaterialTheme.colorScheme.primary
                                    )
                                }

                                val mediaInt = remember { MutableInteractionSource() }
                                IconButton(
                                    onClick = {
                                        showFloatingToolbar = false
                                        filePickerLauncher.launch("image/*")
                                    },
                                    modifier = Modifier.expressivePressScale(mediaInt)
                                ) {
                                    Icon(
                                        imageVector = Icons.Outlined.Image,
                                        contentDescription = "Media",
                                        tint = MaterialTheme.colorScheme.secondary
                                    )
                                }

                                val qrInt = remember { MutableInteractionSource() }
                                IconButton(
                                    onClick = {
                                        showFloatingToolbar = false
                                        activePeer?.let { peer ->
                                            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                            clipboard.setPrimaryClip(ClipData.newPlainText("Peer Ticket", peer.peerKey))
                                            Toast.makeText(context, "Sovereign ticket copied", Toast.LENGTH_SHORT).show()
                                        }
                                    },
                                    modifier = Modifier.expressivePressScale(qrInt)
                                ) {
                                    Icon(
                                        imageVector = Icons.Outlined.QrCode,
                                        contentDescription = "Ticket",
                                        tint = MaterialTheme.colorScheme.tertiary
                                    )
                                }

                                val pingInt = remember { MutableInteractionSource() }
                                IconButton(
                                    onClick = {
                                        activePeer?.let {
                                            viewModel.pingPeer(it.peerKey)
                                            Toast.makeText(context, "Measuring live P2P RTT...", Toast.LENGTH_SHORT).show()
                                        }
                                    },
                                    modifier = Modifier.expressivePressScale(pingInt)
                                ) {
                                    Icon(
                                        imageVector = Icons.Default.Refresh,
                                        contentDescription = "Ping",
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            }
                        }
                    }

                    // Main Input Row with SplitButton Send Action
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        // Quick action launcher button
                        val attachInt = remember { MutableInteractionSource() }
                        FilledTonalIconButton(
                            onClick = { showFloatingToolbar = !showFloatingToolbar },
                            modifier = Modifier
                                .size(40.dp)
                                .expressivePressScale(attachInt),
                            colors = IconButtonDefaults.filledTonalIconButtonColors(
                                containerColor = if (showFloatingToolbar) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh
                            )
                        ) {
                            Icon(
                                imageVector = if (showFloatingToolbar) Icons.Default.Close else Icons.Default.Add,
                                contentDescription = "Quick Actions",
                                modifier = Modifier.size(20.dp)
                            )
                        }

                        Spacer(modifier = Modifier.width(6.dp))

                        // Text input field
                        OutlinedTextField(
                            value = inputText,
                            onValueChange = {
                                inputText = it
                                if (it.isNotEmpty()) viewModel.sendTyping(true)
                            },
                            placeholder = { Text("Encrypted message...") },
                            modifier = Modifier
                                .weight(1f)
                                .padding(horizontal = 2.dp),
                            shape = RoundedCornerShape(22.dp),
                            colors = OutlinedTextFieldDefaults.colors(
                                focusedContainerColor = MaterialTheme.colorScheme.surface,
                                unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerLow
                            ),
                            maxLines = 4
                        )

                        Spacer(modifier = Modifier.width(6.dp))

                        // SplitButton for the main Send Action
                        Box {
                            SplitButtonLayout(
                                leadingButton = {
                                    SplitButtonDefaults.LeadingButton(
                                        onClick = {
                                            if (inputText.isNotBlank()) {
                                                viewModel.sendMessage(inputText)
                                                inputText = ""
                                                showFloatingToolbar = false
                                            }
                                        },
                                        enabled = inputText.isNotBlank()
                                    ) {
                                        Icon(
                                            imageVector = Icons.AutoMirrored.Filled.Send,
                                            contentDescription = "Send message",
                                            modifier = Modifier.size(18.dp)
                                        )
                                    }
                                },
                                trailingButton = {
                                    SplitButtonDefaults.TrailingButton(
                                        checked = showSendMenu,
                                        onCheckedChange = { showSendMenu = it }
                                    ) {
                                        Icon(
                                            imageVector = Icons.Default.ArrowDropDown,
                                            contentDescription = "Send Options",
                                            modifier = Modifier.size(18.dp)
                                        )
                                    }
                                }
                            )

                            DropdownMenu(
                                expanded = showSendMenu,
                                onDismissRequest = { showSendMenu = false }
                            ) {
                                DropdownMenuItem(
                                    text = { Text("Normal Send") },
                                    leadingIcon = {
                                        Icon(
                                            Icons.AutoMirrored.Filled.Send,
                                            contentDescription = null,
                                            modifier = Modifier.size(18.dp)
                                        )
                                    },
                                    onClick = {
                                        showSendMenu = false
                                        if (inputText.isNotBlank()) {
                                            viewModel.sendMessage(inputText)
                                            inputText = ""
                                            showFloatingToolbar = false
                                        }
                                    }
                                )
                                DropdownMenuItem(
                                    text = { Text("Silent Send (Whisper)") },
                                    leadingIcon = {
                                        Icon(
                                            Icons.Outlined.NotificationsOff,
                                            contentDescription = null,
                                            modifier = Modifier.size(18.dp)
                                        )
                                    },
                                    onClick = {
                                        showSendMenu = false
                                        if (inputText.isNotBlank()) {
                                            viewModel.sendMessage("[silent] $inputText")
                                            inputText = ""
                                            showFloatingToolbar = false
                                        }
                                    }
                                )
                                DropdownMenuItem(
                                    text = { Text("Urgent Send (Priority)") },
                                    leadingIcon = {
                                        Icon(
                                            Icons.Outlined.PriorityHigh,
                                            contentDescription = null,
                                            modifier = Modifier.size(18.dp)
                                        )
                                    },
                                    onClick = {
                                        showSendMenu = false
                                        if (inputText.isNotBlank()) {
                                            viewModel.sendMessage("[urgent] $inputText")
                                            inputText = ""
                                            showFloatingToolbar = false
                                        }
                                    }
                                )
                                HorizontalDivider()
                                DropdownMenuItem(
                                    text = { Text("Clear Draft") },
                                    leadingIcon = {
                                        Icon(
                                            Icons.Default.Close,
                                            contentDescription = null,
                                            modifier = Modifier.size(18.dp)
                                        )
                                    },
                                    onClick = {
                                        showSendMenu = false
                                        inputText = ""
                                    }
                                )
                            }
                        }
                    }
                }
            }
        }
    ) { innerPadding ->
        LazyColumn(
            state = listState,
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(horizontal = 12.dp),
            contentPadding = PaddingValues(vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            items(state.messages, key = { it.id }) { message ->
                MessageBubble(
                    message = message,
                    isSelected = selectedMessageId == message.id,
                    onSelect = {
                        selectedMessageId = if (selectedMessageId == message.id) null else message.id
                    },
                    context = context
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MessageBubble(
    message: MessageItem,
    isSelected: Boolean,
    onSelect: () -> Unit,
    context: Context
) {
    val isMine = message.isMine

    // State-driven corner shape morphing using slowSpatialSpec
    val topStartMorph by animateDpAsState(
        targetValue = when {
            isSelected && !isMine -> 8.dp
            isSelected && isMine -> 28.dp
            else -> 20.dp
        },
        animationSpec = MaterialTheme.motionScheme.slowSpatialSpec(),
        label = "top_start_morph"
    )

    val topEndMorph by animateDpAsState(
        targetValue = when {
            isSelected && isMine -> 8.dp
            isSelected && !isMine -> 28.dp
            else -> 20.dp
        },
        animationSpec = MaterialTheme.motionScheme.slowSpatialSpec(),
        label = "top_end_morph"
    )

    val bottomStartMorph by animateDpAsState(
        targetValue = when {
            isSelected -> 28.dp
            !isMine -> 4.dp
            else -> 20.dp
        },
        animationSpec = MaterialTheme.motionScheme.slowSpatialSpec(),
        label = "bottom_start_morph"
    )

    val bottomEndMorph by animateDpAsState(
        targetValue = when {
            isSelected -> 28.dp
            isMine -> 4.dp
            else -> 20.dp
        },
        animationSpec = MaterialTheme.motionScheme.slowSpatialSpec(),
        label = "bottom_end_morph"
    )

    val bubbleShape = RoundedCornerShape(
        topStart = topStartMorph,
        topEnd = topEndMorph,
        bottomStart = bottomStartMorph,
        bottomEnd = bottomEndMorph
    )

    // Color shift animation using fastEffectsSpec
    val targetContainerColor = when {
        isSelected -> MaterialTheme.colorScheme.secondaryContainer
        isMine -> MaterialTheme.colorScheme.primaryContainer
        else -> MaterialTheme.colorScheme.surfaceContainerHigh
    }

    val containerColor by animateColorAsState(
        targetValue = targetContainerColor,
        animationSpec = MaterialTheme.motionScheme.fastEffectsSpec(),
        label = "bubble_color"
    )

    val contentColor = when {
        isSelected -> MaterialTheme.colorScheme.onSecondaryContainer
        isMine -> MaterialTheme.colorScheme.onPrimaryContainer
        else -> MaterialTheme.colorScheme.onSurface
    }

    val elevation by animateDpAsState(
        targetValue = if (isSelected) 6.dp else 2.dp,
        animationSpec = MaterialTheme.motionScheme.fastEffectsSpec(),
        label = "bubble_elevation"
    )

    val interactionSource = remember { MutableInteractionSource() }

    Box(
        modifier = Modifier.fillMaxWidth(),
        contentAlignment = if (isMine) Alignment.CenterEnd else Alignment.CenterStart
    ) {
        Column(
            horizontalAlignment = if (isMine) Alignment.End else Alignment.Start,
            modifier = Modifier.widthIn(max = 310.dp)
        ) {
            Surface(
                shape = bubbleShape,
                color = containerColor,
                tonalElevation = elevation,
                modifier = Modifier
                    .expressivePressScale(interactionSource, pressedScale = 0.95f)
                    .clickable(
                        interactionSource = interactionSource,
                        indication = null,
                        onClick = onSelect
                    )
            ) {
                Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp)) {
                    if (message.attachment != null) {
                        AttachmentCard(attachment = message.attachment, context = context)
                        Spacer(modifier = Modifier.height(6.dp))
                    }

                    if (message.content.isNotBlank()) {
                        Text(
                            text = message.content,
                            style = MaterialTheme.typography.bodyLarge,
                            color = contentColor
                        )
                    }

                    Spacer(modifier = Modifier.height(4.dp))
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.End,
                        modifier = Modifier.align(Alignment.End)
                    ) {
                        Text(
                            text = message.timestamp.takeLast(8),
                            style = MaterialTheme.typography.labelSmall,
                            color = contentColor.copy(alpha = 0.65f),
                            fontSize = 10.sp
                        )
                        if (isMine) {
                            Spacer(modifier = Modifier.width(4.dp))
                            if (message.status.equals("Pending", ignoreCase = true)) {
                                LoadingIndicator(
                                    modifier = Modifier.size(10.dp),
                                    color = contentColor.copy(alpha = 0.7f)
                                )
                            } else {
                                val tickIcon = if (message.status.equals("Delivered", ignoreCase = true)) {
                                    Icons.Default.DoneAll
                                } else {
                                    Icons.Default.Done
                                }
                                Icon(
                                    imageVector = tickIcon,
                                    contentDescription = message.status,
                                    modifier = Modifier.size(13.dp),
                                    tint = contentColor.copy(alpha = 0.7f)
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AttachmentCard(
    attachment: AttachmentItem,
    context: Context
) {
    val interactionSource = remember { MutableInteractionSource() }

    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.tertiaryContainer,
        tonalElevation = 2.dp,
        modifier = Modifier
            .fillMaxWidth()
            .expressivePressScale(interactionSource, pressedScale = 0.95f)
            .clickable(
                interactionSource = interactionSource,
                indication = null
            ) {
                attachment.localPath?.let { path ->
                    openFileIntent(context, path, attachment.mimeType)
                } ?: run {
                    Toast.makeText(context, "Receiving file over QUIC...", Toast.LENGTH_SHORT).show()
                }
            }
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(42.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.6f)),
                contentAlignment = Alignment.Center
            ) {
                if (attachment.localPath == null) {
                    LoadingIndicator(
                        modifier = Modifier.size(24.dp),
                        color = MaterialTheme.colorScheme.onTertiaryContainer
                    )
                } else {
                    Icon(
                        imageVector = if (attachment.isDirectory) Icons.Default.Folder else Icons.AutoMirrored.Filled.InsertDriveFile,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onTertiaryContainer,
                        modifier = Modifier.size(24.dp)
                    )
                }
            }
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = attachment.fileName,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onTertiaryContainer,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                val sizeText = formatFileSize(attachment.fileSize)
                Text(
                    text = if (attachment.isDirectory) "Directory • $sizeText" else sizeText,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onTertiaryContainer.copy(alpha = 0.8f)
                )
            }
        }
    }
}

fun formatFileSize(bytes: Long): String {
    if (bytes <= 0) return "0 B"
    val kb = bytes / 1024.0
    val mb = kb / 1024.0
    return when {
        mb >= 1.0 -> String.format("%.1f MB", mb)
        kb >= 1.0 -> String.format("%.1f KB", kb)
        else -> "$bytes B"
    }
}

fun copyUriToCache(context: Context, uri: Uri): String? {
    return try {
        var fileName = "file_${System.currentTimeMillis()}"
        context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (idx >= 0) fileName = cursor.getString(idx)
            }
        }
        val destFile = File(context.cacheDir, fileName)
        context.contentResolver.openInputStream(uri)?.use { input ->
            destFile.outputStream().use { output ->
                input.copyTo(output)
            }
        }
        destFile.absolutePath
    } catch (e: Exception) {
        e.printStackTrace()
        null
    }
}

fun openFileIntent(context: Context, path: String, mimeType: String) {
    try {
        val file = File(path)
        if (!file.exists()) {
            Toast.makeText(context, "File does not exist on disk", Toast.LENGTH_SHORT).show()
            return
        }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, mimeType.ifBlank { "*/*" })
            flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK
        }
        context.startActivity(intent)
    } catch (e: Exception) {
        Toast.makeText(context, "No app available to open this file", Toast.LENGTH_SHORT).show()
    }
}
