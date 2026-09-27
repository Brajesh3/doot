use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jstring};
use message_core::{MessengerCommand, MessengerHandle};
use parking_lot::Mutex;
use std::path::PathBuf;
use tokio::runtime::Runtime;

struct EngineState {
    _rt: Runtime,
    handle: MessengerHandle,
}

static ENGINE: Mutex<Option<EngineState>> = Mutex::new(None);

// -----------------------------------------------------------------------------
// initEngine
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_initEngine(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    nickname: JString,
) -> jboolean {
    let data_dir_str: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    let nickname_str: String = match env.get_string(&nickname) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to create Tokio runtime: {:?}", e);
            return JNI_FALSE;
        }
    };

    let dir = PathBuf::from(data_dir_str);
    let nick = if nickname_str.is_empty() {
        None
    } else {
        Some(nickname_str)
    };

    let handle_res = rt.block_on(async { MessengerHandle::start(Some(dir), nick).await });

    match handle_res {
        Ok(handle) => {
            let mut lock = ENGINE.lock();
            *lock = Some(EngineState { _rt: rt, handle });
            JNI_TRUE
        }
        Err(e) => {
            eprintln!("Failed to start MessengerHandle: {:?}", e);
            JNI_FALSE
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_initEngine(
    env: JNIEnv,
    class: JClass,
    data_dir: JString,
    nickname: JString,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_initEngine(env, class, data_dir, nickname)
}

// -----------------------------------------------------------------------------
// getMyTicket
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_getMyTicket(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let lock = ENGINE.lock();
    let ticket = match lock.as_ref() {
        Some(state) => state.handle.my_ticket.clone(),
        None => String::new(),
    };
    drop(lock);

    env.new_string(ticket)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMyTicket(
    env: JNIEnv,
    class: JClass,
) -> jstring {
    Java_com_brajesh_doot_messenger_MessengerBridge_getMyTicket(env, class)
}

// -----------------------------------------------------------------------------
// getMyNodeId
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_getMyNodeId(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let lock = ENGINE.lock();
    let node_id = match lock.as_ref() {
        Some(state) => state.handle.my_node_id.clone(),
        None => String::new(),
    };
    drop(lock);

    env.new_string(node_id)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMyNodeId(
    env: JNIEnv,
    class: JClass,
) -> jstring {
    Java_com_brajesh_doot_messenger_MessengerBridge_getMyNodeId(env, class)
}

// -----------------------------------------------------------------------------
// getMyNickname
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_getMyNickname(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let lock = ENGINE.lock();
    let nick = match lock.as_ref() {
        Some(state) => state.handle.my_nickname.clone(),
        None => String::new(),
    };
    drop(lock);

    env.new_string(nick)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMyNickname(
    env: JNIEnv,
    class: JClass,
) -> jstring {
    Java_com_brajesh_doot_messenger_MessengerBridge_getMyNickname(env, class)
}

// -----------------------------------------------------------------------------
// connectPeer
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_connectPeer(
    mut env: JNIEnv,
    _class: JClass,
    ticket_or_id: JString,
    nickname: JString,
) -> jboolean {
    let ticket_str: String = match env.get_string(&ticket_or_id) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    let nick_str: String = match env.get_string(&nickname) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let opt_nick = if nick_str.is_empty() {
        None
    } else {
        Some(nick_str)
    };

    let lock = ENGINE.lock();
    if let Some(state) = lock.as_ref() {
        let res = state.handle.send_command(MessengerCommand::ConnectPeer {
            ticket_or_id: ticket_str,
            nickname: opt_nick,
        });
        if res.is_ok() {
            return JNI_TRUE;
        }
    }
    JNI_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_connectPeer(
    env: JNIEnv,
    class: JClass,
    ticket_or_id: JString,
    nickname: JString,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_connectPeer(env, class, ticket_or_id, nickname)
}

// -----------------------------------------------------------------------------
// sendTextMessage
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_sendTextMessage(
    mut env: JNIEnv,
    _class: JClass,
    peer_key: JString,
    content: JString,
) -> jboolean {
    let peer_str: String = match env.get_string(&peer_key) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    let content_str: String = match env.get_string(&content) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let lock = ENGINE.lock();
    if let Some(state) = lock.as_ref() {
        let res = state
            .handle
            .send_command(MessengerCommand::SendTextMessage {
                peer_key: peer_str,
                content: content_str,
            });
        if res.is_ok() {
            return JNI_TRUE;
        }
    }
    JNI_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_sendTextMessage(
    env: JNIEnv,
    class: JClass,
    peer_key: JString,
    content: JString,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_sendTextMessage(env, class, peer_key, content)
}

// -----------------------------------------------------------------------------
// sendTyping
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_sendTyping(
    mut env: JNIEnv,
    _class: JClass,
    peer_key: JString,
    is_typing: jboolean,
) -> jboolean {
    let peer_str: String = match env.get_string(&peer_key) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let lock = ENGINE.lock();
    if let Some(state) = lock.as_ref() {
        let res = state.handle.send_command(MessengerCommand::SendTyping {
            peer_key: peer_str,
            is_typing: is_typing == JNI_TRUE,
        });
        if res.is_ok() {
            return JNI_TRUE;
        }
    }
    JNI_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_sendTyping(
    env: JNIEnv,
    class: JClass,
    peer_key: JString,
    is_typing: jboolean,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_sendTyping(env, class, peer_key, is_typing)
}

// -----------------------------------------------------------------------------
// getContactsJson
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_getContactsJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let lock = ENGINE.lock();
    let contacts = match lock.as_ref() {
        Some(state) => state.handle.get_contacts(),
        None => Vec::new(),
    };
    drop(lock);

    let json = serde_json::to_string(&contacts).unwrap_or_else(|_| "[]".to_string());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getContactsJson(
    env: JNIEnv,
    class: JClass,
) -> jstring {
    Java_com_brajesh_doot_messenger_MessengerBridge_getContactsJson(env, class)
}

// -----------------------------------------------------------------------------
// getMessagesJson
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_getMessagesJson(
    mut env: JNIEnv,
    _class: JClass,
    peer_key: JString,
) -> jstring {
    let peer_str: String = match env.get_string(&peer_key) {
        Ok(s) => s.into(),
        Err(_) => String::new(),
    };

    let lock = ENGINE.lock();
    let msgs = match lock.as_ref() {
        Some(state) => state.handle.get_messages(&peer_str),
        None => Vec::new(),
    };
    drop(lock);

    let json = serde_json::to_string(&msgs).unwrap_or_else(|_| "[]".to_string());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMessagesJson(
    env: JNIEnv,
    class: JClass,
    peer_key: JString,
) -> jstring {
    Java_com_brajesh_doot_messenger_MessengerBridge_getMessagesJson(env, class, peer_key)
}

// -----------------------------------------------------------------------------
// pollEventsJson
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_pollEventsJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let lock = ENGINE.lock();
    let mut events = Vec::new();
    if let Some(state) = lock.as_ref() {
        while let Some(evt) = state.handle.try_recv_event() {
            events.push(evt);
        }
    }
    drop(lock);

    let json = serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_string());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_pollEventsJson(
    env: JNIEnv,
    class: JClass,
) -> jstring {
    Java_com_brajesh_doot_messenger_MessengerBridge_pollEventsJson(env, class)
}

// -----------------------------------------------------------------------------
// markAsRead
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_markAsRead(
    mut env: JNIEnv,
    _class: JClass,
    peer_key: JString,
) -> jboolean {
    let peer_str: String = match env.get_string(&peer_key) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let lock = ENGINE.lock();
    if let Some(state) = lock.as_ref() {
        let _ = state.handle.mark_as_read(&peer_str);
        return JNI_TRUE;
    }
    JNI_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_markAsRead(
    env: JNIEnv,
    class: JClass,
    peer_key: JString,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_markAsRead(env, class, peer_key)
}

// -----------------------------------------------------------------------------
// sendFile
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_sendFile(
    mut env: JNIEnv,
    _class: JClass,
    peer_key: JString,
    path: JString,
    is_directory: jboolean,
) -> jboolean {
    let peer_str: String = match env.get_string(&peer_key) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    let path_str: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let lock = ENGINE.lock();
    if let Some(state) = lock.as_ref() {
        let res =
            state
                .handle
                .send_file(&peer_str, PathBuf::from(path_str), is_directory == JNI_TRUE);
        if res.is_ok() {
            return JNI_TRUE;
        }
    }
    JNI_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_sendFile(
    env: JNIEnv,
    class: JClass,
    peer_key: JString,
    path: JString,
    is_directory: jboolean,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_sendFile(
        env,
        class,
        peer_key,
        path,
        is_directory,
    )
}

// -----------------------------------------------------------------------------
// pingPeer
// -----------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_brajesh_doot_messenger_MessengerBridge_pingPeer(
    mut env: JNIEnv,
    _class: JClass,
    peer_key: JString,
) -> jboolean {
    let peer_str: String = match env.get_string(&peer_key) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let lock = ENGINE.lock();
    if let Some(state) = lock.as_ref() {
        let res = state
            .handle
            .send_command(MessengerCommand::PingPeer { peer_key: peer_str });
        if res.is_ok() {
            return JNI_TRUE;
        }
    }
    JNI_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_pingPeer(
    env: JNIEnv,
    class: JClass,
    peer_key: JString,
) -> jboolean {
    Java_com_brajesh_doot_messenger_MessengerBridge_pingPeer(env, class, peer_key)
}
