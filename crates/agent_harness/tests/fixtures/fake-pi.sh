#!/bin/sh

emit() { printf '%s\n' "$1"; }
rid() { printf '%s' "$1" | sed 's/.*"id":\([0-9]*\).*/\1/'; }
has() { case "$1" in *"$2"*) return 0 ;; *) return 1 ;; esac; }
option_id() { printf '%s' "$1" | sed 's/.*"optionId":"\([^"]*\)".*/\1/'; }

update() { # $1 = sessionUpdate kind, $2 = payload
  emit "{\"method\":\"session/update\",\"params\":{\"sessionId\":\"s-1\",\"update\":{\"sessionUpdate\":\"$1\"$2}}}"
}

respond() { # $1 = request line, $2 = result JSON
  emit "{\"id\":$(rid "$1"),\"result\":$2}"
}

pending=""
while :; do
  if [ -n "$pending" ]; then
    line="$pending"
    pending=""
  else
    read -r line || exit 0
  fi
  if has "$line" '"method":"initialize"'; then
    respond "$line" '{"protocolVersion":1,"authMethods":[{"type":"agent","id":"agent","name":"Use existing local credentials"}],"agentCapabilities":{"loadSession":true},"agentInfo":{"name":"pi-acp","version":"0.0.0-fake"}}'
    continue
  fi
  if has "$line" '"method":"authenticate"'; then
    respond "$line" '{}'
    continue
  fi
  if has "$line" '"method":"session/load"'; then
    respond "$line" '{"modes":{"currentModeId":"default","availableModes":[{"id":"default","name":"Default"}]},"configOptions":[]}'
    update user_message_chunk ',"content":{"type":"text","text":"earlier"}'
    continue
  fi
  if has "$line" '"method":"session/new"'; then
    respond "$line" '{"sessionId":"s-1","modes":{"currentModeId":"default","availableModes":[{"id":"default","name":"Default"}]},"configOptions":[{"id":"model","name":"Model","category":"model","type":"select","currentValue":"m1","options":[{"value":"m1","name":"Model One","description":"provider/a"},{"value":"m2","name":"Model Two","description":"provider/b"}]},{"id":"thought_level","name":"Thinking","category":"thought_level","type":"select","currentValue":"auto","options":[{"value":"off","name":"Off"},{"value":"low","name":"Low"},{"value":"medium","name":"Medium"},{"value":"high","name":"High"},{"value":"xhigh","name":"X-High"}]}]}'
    # A catalog-only run never sends follow-up requests: exit after a quiet window.
    if read -t 5 pending; then
      continue
    fi
    exit 0
  fi
  if has "$line" '"method":"session/set_config_option"'; then
    has "$line" '"configId"' || { emit "{\"id\":$(rid \"$line\"),\"error\":{\"code\":-32602,\"message\":\"configId missing\"}}"; continue; }
    respond "$line" '{}'
    continue
  fi
  if has "$line" '"method":"session/prompt"'; then
    prompt_id=$(rid "$line")
    case "$line" in
      *scenario:happy*)
        update agent_thought_chunk ',"content":{"type":"text","text":"thinking"}'
        update agent_message_chunk ',"content":{"type":"text","text":"Hello"}'
        update agent_message_chunk ',"content":{"type":"text","text":" world"}'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\",\"usage\":{\"totalTokens\":900,\"inputTokens\":700,\"outputTokens\":200}}}"
        ;;
      *scenario:tools*)
        update tool_call ',"toolCallId":"c1","title":"ls -la","kind":"execute","status":"in_progress","rawInput":{"command":"ls -la"}'
        update tool_call_update ',"toolCallId":"c1","status":"completed","content":[{"type":"content","content":{"type":"text","text":"file.txt"}}]'
        update tool_call ',"toolCallId":"c2","title":"Edit /w/f.txt","kind":"edit","status":"in_progress","locations":[{"path":"/w/f.txt"}],"rawInput":{"path":"/w/f.txt"}'
        update tool_call_update ',"toolCallId":"c2","status":"completed","content":[{"type":"diff","path":"/w/f.txt","oldText":"old","newText":"new"}]'
        update tool_call ',"toolCallId":"c3","title":"grep TODO","kind":"search","status":"in_progress","rawInput":{"pattern":"TODO"}'
        update tool_call_update ',"toolCallId":"c3","status":"failed","content":[{"type":"content","content":{"type":"text","text":"boom"}}]'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\"}}"
        ;;
      *scenario:plan*)
        update plan ',"entries":[{"content":"read the code","status":"completed"},{"content":"fix the bug","status":"in_progress"},{"content":"run tests","status":"pending"}]'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\"}}"
        ;;
      *scenario:permission*)
        update tool_call ',"toolCallId":"c1","title":"ls -la","kind":"execute","status":"pending","rawInput":{"command":"ls -la"}'
        emit '{"jsonrpc":"2.0","id":"p1","method":"session/request_permission","params":{"sessionId":"s-1","toolCall":{"toolCallId":"c1","title":"ls -la","kind":"execute"},"options":[{"optionId":"yes","name":"Yes","kind":"allow_once"},{"optionId":"no","name":"No","kind":"reject_once"}]}}'
        read -r answer || exit 1
        if has "$answer" '"selected"'; then
          chosen=$(option_id "$answer")
        else
          chosen=reject_once
        fi
        update tool_call_update ',"toolCallId":"c1","status":"completed","content":[{"type":"content","content":{"type":"text","text":"'"$chosen"'"}}]'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\"}}"
        ;;
      *scenario:steer*)
        update agent_message_chunk ',"content":{"type":"text","text":"a"}'
        read -r steer_line || exit 1
        has "$steer_line" '_session/steering' || exit 1
        respond "$steer_line" '{}'
        update agent_message_chunk ',"content":{"type":"text","text":"b"}'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\"}}"
        ;;
      *scenario:cancel*)
        read -r cancel_line || exit 1
        has "$cancel_line" '"method":"session/cancel"' || exit 1
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"cancelled\"}}"
        ;;
      *scenario:fs*)
        update tool_call ',"toolCallId":"c1","title":"Read /etc/hosts","kind":"read","status":"in_progress","locations":[{"path":"/etc/hosts"}]'
        emit '{"jsonrpc":"2.0","id":"r1","method":"fs/read_text_file","params":{"sessionId":"s-1","path":"/etc/hosts"}}'
        read -r answer || exit 1
        has "$answer" '"content"' || exit 1
        update tool_call_update ',"toolCallId":"c1","status":"completed"'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\"}}"
        ;;
      *)
        update agent_message_chunk ',"content":{"type":"text","text":"plain"}'
        emit "{\"id\":$prompt_id,\"result\":{\"stopReason\":\"end_turn\"}}"
        ;;
    esac
    continue
  fi
  if has "$line" '"method":"_session/steering"'; then
    # Steering arriving with no owning prompt is refused so Wu resubmits it.
    emit "{\"id\":$(rid "$line"),\"error\":{\"code\":-32000,\"message\":\"promptRequired\"}}"
    continue
  fi
done
