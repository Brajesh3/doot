use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use message_core::{MessengerCommand, MessengerEvent, MessengerHandle};
use parking_lot::Mutex;
use std::path::PathBuf;
use tokio::runtime::Runtime;

struct EngineState {
    _rt: Runtime,
    handle: MessengerHandle,
}

static ENGINE: Mutex<Option<EngineState>> = Mutex::new(None);

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_initEngine(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMyTicket(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMyNodeId(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMyNickname(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_connectPeer(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_sendTextMessage(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_sendTyping(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getContactsJson(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_getMessagesJson(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_pollEventsJson(
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

#[no_mangle]
pub extern "system" fn Java_com_example_testapp_messenger_MessengerBridge_markAsRead(
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
