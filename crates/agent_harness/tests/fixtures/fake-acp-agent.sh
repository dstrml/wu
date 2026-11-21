#!/bin/sh

emit() { printf '%s\n' "$1"; }
rid() { printf '%s' "$1" | sed 's/.*"id":\([0-9]*\).*/\1/'; }
field() { printf '%s' "$1" | sed "s/.*\"$2\":\"\([^\"]*\)\".*/\1/"; }
has() { case "$1" in *"$2"*) return 0 ;; *) return 1 ;; esac; }

[ "$1" = "acp" ] || exit 2
legacy=false
[ "$2" = "--legacy" ] && legacy=true

model=gemini-flash
level=high
mode=default
sid=s1
if $legacy; then
  model=m1
  mode=ask
  level=none
fi

config() {
  levels=',{"id":"thought_level","name":"Thinking Level","category":"thought_level","type":"select","currentValue":"'$level'","options":[{"value":"low","name":"Low"},{"value":"high","name":"High"}]}'
  [ "$model" = "claude-opus" ] && levels=''
  printf '%s' '[{"id":"model","name":"Model","category":"model","type":"select","currentValue":"'$model'","options":[{"value":"gemini-flash","name":"Gemini Flash"},{"value":"claude-opus","name":"Claude Opus"}]}'"$levels"',{"id":"mode","name":"Mode","category":"mode","type":"select","currentValue":"'$mode'","options":[{"value":"default","name":"Default"},{"value":"accept-edits","name":"Accept edits"},{"value":"auto","name":"Auto"}]}]'
}

update() { emit '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"'$sid'","update":'"$1"'}}'; }
text() { update '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"'"$1"'"}}'; }

read -r line || exit 1
has "$line" '"method":"initialize"' && has "$line" '"protocolVersion":1' && has "$line" '"name":"wu"' || exit 1
if $legacy; then
  emit '{"jsonrpc":"2.0","id":'$(rid "$line")',"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true},"authMethods":[]}}'
else
  emit '{"jsonrpc":"2.0","id":'$(rid "$line")',"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"sessionCapabilities":{"resume":{},"list":{}}},"authMethods":[]}}'
fi

while read -r line; do
  id=$(rid "$line")
  case "$line" in
    *'"method":"session/new"'*)
      if has "$line" '"cwd":""'; then
        emit '{"jsonrpc":"2.0","id":'$id',"error":{"code":-32602,"message":"cwd must be absolute"}}'
        continue
      fi
      if has "$line" 'signed-out'; then
        emit '{"jsonrpc":"2.0","id":'$id',"error":{"code":-32000,"message":"Authentication required"}}'
        continue
      fi
      if $legacy; then
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"sessionId":"'$sid'","models":{"availableModels":[{"modelId":"m1","name":"One"},{"modelId":"m2","name":"Two"}],"currentModelId":"'$model'"},"modes":{"availableModes":[{"id":"ask","name":"Ask"},{"id":"yolo","name":"Yolo"}],"currentModeId":"'$mode'"}}}'
      else
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"sessionId":"'$sid'","configOptions":'"$(config)"'}}'
      fi
      if has "$line" 'commands'; then
        update '{"sessionUpdate":"available_commands_update","availableCommands":[{"name":"goal","description":"Run until the goal is done."}]}'
      fi
      ;;
    *'"method":"session/resume"'*)
      if has "$line" '"sessionId":"gone"'; then
        emit '{"jsonrpc":"2.0","id":'$id',"error":{"code":-32602,"message":"Session not found"}}'
        continue
      fi
      sid=$(field "$line" sessionId)
      emit '{"jsonrpc":"2.0","id":'$id',"result":{"configOptions":'"$(config)"'}}'
      ;;
    *'"method":"session/set_config_option"'*)
      value=$(field "$line" value)
      case "$(field "$line" configId)" in
        model) model=$value ;;
        thought_level) level=$value ;;
        mode) mode=$value ;;
      esac
      emit '{"jsonrpc":"2.0","id":'$id',"result":{"configOptions":'"$(config)"'}}'
      ;;
    *'"method":"session/set_model"'*)
      model=$(field "$line" modelId)
      emit '{"jsonrpc":"2.0","id":'$id',"result":{}}'
      ;;
    *'"method":"session/set_mode"'*)
      mode=$(field "$line" modeId)
      emit '{"jsonrpc":"2.0","id":'$id',"result":{}}'
      ;;
    *'"method":"session/list"'*)
      emit '{"jsonrpc":"2.0","id":'$id',"result":{"sessions":[{"sessionId":"old","cwd":"/w","title":"Fix the build","updatedAt":"2026-11-21T17:25:54Z"}]}}'
      ;;
    *'"method":"session/load"'*)
      sid=$(field "$line" sessionId)
      update '{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"Fix the build"}}'
      update '{"sessionUpdate":"tool_call","toolCallId":"c1","name":"run_command","kind":"execute","status":"completed","title":"make","rawInput":{"CommandLine":"make"},"rawOutput":{"output":"ok","exit_code":0}}'
      text 'Fixed.'
      update '{"sessionUpdate":"usage_update","used":5,"size":10}'
      emit '{"jsonrpc":"2.0","id":'$id',"result":{"configOptions":'"$(config)"'}}'
      ;;
    *'"method":"session/prompt"'*)
      if has "$line" '"text":"hello"'; then
        text "model=$model level=$level mode=$mode"
        update '{"sessionUpdate":"tool_call","toolCallId":"c1","name":"run_command","kind":"execute","status":"pending","title":"ls","rawInput":{"CommandLine":"ls"}}'
        emit '{"jsonrpc":"2.0","id":"p1","method":"session/request_permission","params":{"sessionId":"'$sid'","toolCall":{"toolCallId":"c1","name":"run_command","kind":"execute","title":"ls","rawInput":{"CommandLine":"ls"}},"options":[{"optionId":"yes","name":"Allow","kind":"allow_once"},{"optionId":"no","name":"Deny","kind":"reject_once"}]}}'
        read -r reply || exit 1
        if has "$reply" '"optionId":"yes"'; then
          update '{"sessionUpdate":"tool_call_update","toolCallId":"c1","kind":"execute","status":"completed","rawOutput":{"output":"Command exited with code 0.\n\nCommand output:\nnotes.txt\r\n","exit_code":0}}'
          text 'allowed'
        else
          update '{"sessionUpdate":"tool_call_update","toolCallId":"c1","kind":"execute","status":"failed"}'
          text 'denied'
        fi
        update '{"sessionUpdate":"usage_update","used":900,"size":1000}'
        update '{"sessionUpdate":"session_info_update","title":"List files"}'
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"stopReason":"end_turn","usage":{"inputTokens":12,"outputTokens":3}}}'
      elif has "$line" '"text":"steer"'; then
        text 'first'
        read -r second || exit 1
        has "$second" '"text":"and more"' || exit 1
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"stopReason":"end_turn"}}'
        text 'second'
        emit '{"jsonrpc":"2.0","id":'$(rid "$second")',"result":{"stopReason":"end_turn"}}'
      elif has "$line" '"text":"slow"'; then
        text 'first'
        sleep 1
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"stopReason":"end_turn"}}'
      elif has "$line" '"text":"wait"'; then
        text 'waiting'
        read -r cancel || exit 1
        has "$cancel" '"method":"session/cancel"' || exit 1
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"stopReason":"cancelled"}}'
      elif has "$line" '"text":"crash"'; then
        echo "agy: lost connection" >&2
        exit 3
      else
        text "said: $(field "$line" text)"
        emit '{"jsonrpc":"2.0","id":'$id',"result":{"stopReason":"end_turn"}}'
      fi
      ;;
  esac
done
