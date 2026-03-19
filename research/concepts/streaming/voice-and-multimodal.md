# Voice and Multimodal Streaming

Beyond text: voice coding, multimodal inputs, and future streaming modalities.

---

## 1. Voice Coding with Aider

### How It Works

- Speech-to-text pipeline: microphone → transcription → text prompt
- Aider's `--voice` flag activates voice mode
- Uses OpenAI Whisper API for transcription
- Flow: Record → Transcribe → Send to LLM → Stream response → Apply edits
- User speaks natural language coding instructions
- LLM interprets and generates code edits
- The voice input substitutes for typed prompts — all downstream processing remains identical

### Technical Architecture

- Audio capture via system microphone using platform APIs
- Audio format: WAV or WebM depending on platform
- Whisper API call: `POST /v1/audio/transcriptions` with audio file
- Transcription latency: typically 1-3 seconds for short utterances
- Combined with Aider's edit format parsing (search/replace blocks, whole files, etc.)
- Voice input replaces keyboard for prompt entry only — the edit loop is unchanged
- No persistent audio connection; each utterance is a discrete API call

### Benefits

- Hands-free coding for ergonomic relief
- Natural language instructions feel more intuitive for describing intent
- Faster for describing complex, multi-step changes than typing
- Accessibility for motor-impaired developers who cannot use a keyboard effectively
- Reduces context-switching between thinking and typing
- Pairs well with standing desks and non-traditional workstations

### Limitations

- Transcription errors, especially for code-specific terms (variable names, APIs, syntax)
- Background noise sensitivity degrades transcription accuracy
- No voice output — responses remain text-only in the terminal
- Cumulative latency: voice capture → transcription → LLM generation → response streaming
- Better for high-level instructions than dictating exact code syntax
- Whisper API costs add to per-request expense
- No streaming of transcription — must wait for full utterance to complete

---

## 2. OpenAI Realtime API

### Architecture

- WebSocket connection: `wss://api.openai.com/v1/realtime`
- Bidirectional: client sends audio/text, server sends audio/text simultaneously
- Event-based protocol over WebSocket frames
- Persistent connection for the duration of a conversation session
- Supports both text and audio modalities in a single session
- Model: `gpt-4o-realtime-preview` (purpose-built for low-latency voice)

### Session Events (Client → Server)

- `session.create` / `session.update` — configure session parameters, tools, voice
- `input_audio_buffer.append` — send raw audio data chunks
- `input_audio_buffer.commit` — signal that the audio input is complete
- `input_audio_buffer.clear` — discard buffered audio
- `conversation.item.create` — inject text or audio items into conversation
- `response.create` — explicitly trigger a model response
- `response.cancel` — cancel an in-progress response

### Server Events (Server → Client)

- `session.created` / `session.updated` — confirm session configuration
- `input_audio_buffer.speech_started` — VAD detected speech onset
- `input_audio_buffer.speech_stopped` — VAD detected speech end
- `response.audio.delta` — audio output chunks (base64-encoded PCM)
- `response.audio_transcript.delta` — text transcript of the audio being generated
- `response.text.delta` — text output chunks (when in text mode)
- `response.function_call_arguments.delta` — streaming tool call arguments
- `response.done` — response generation complete
- `rate_limits.updated` — current rate limit status
- `error` — error information

### Audio Formats

- **PCM16**: 16-bit PCM audio at 24kHz sample rate, little-endian byte order
- **G.711 μ-law**: 8kHz telephony encoding, smaller payloads, lower quality
- **G.711 A-law**: alternative telephony encoding used in European systems
- Input and output formats are configured independently in the session

### Voice Activity Detection (VAD)

- **Server-side VAD**: the API detects when the user starts and stops speaking
  - Automatically commits audio buffer on silence detection
  - Configurable silence threshold (duration of silence to trigger commit)
  - Prefix padding: how much audio before speech onset to include
  - Suffix padding: how much silence after speech end before committing
- **Client-side VAD**: the application controls audio boundaries explicitly
  - Application calls `input_audio_buffer.commit` manually
  - Useful when VAD needs to be customized or combined with UI controls
- `turn_detection` configuration in session setup selects the mode

### Function Calling in Realtime

- Define tools in session configuration (same schema as Chat Completions)
- Model can invoke functions mid-conversation during voice interaction
- Function call arguments arrive via `response.function_call_arguments.delta` events
- Client-side code executes the function and sends the result back
- Conversation continues seamlessly with the function result in context
- Enables voice-driven tool use: "run my tests", "search for that function", etc.

### Use Cases for Coding Agents

- Voice-driven code generation and editing
- Real-time pair programming with bidirectional voice
- Debugging via verbal description of symptoms
- Code review discussion with voice Q&A about diffs
- Hands-free CI/CD pipeline management
- Architecture brainstorming sessions

### Codex CLI and Realtime API

- Codex CLI has a `supports_websockets` flag per provider configuration
- Explicit WebSocket transport support for realtime streaming mode
- Integration with the Responses API streaming infrastructure
- Potential for voice-first coding workflows built on Codex

---

## 3. Multimodal Streaming: Images in Coding Context

### Screenshot/Image Input

- Send screenshots of error messages, UI states, or design mockups
- LLM analyzes the image content and generates corresponding code
- Useful for: UI matching (screenshot → CSS/HTML), error debugging, diagram-to-code
- Images are embedded in the message content array alongside text

### How Streaming Works with Images

- Images are sent as base64-encoded data in the request payload (not streamed incrementally)
- The response streams back as normal text tokens via SSE or WebSocket
- Larger image payloads → longer time-to-first-token due to vision processing
- Image tokens count toward the model's context window (token cost varies by resolution)
- Multiple images can be sent in a single request for comparison or context

### Agent Support for Image Input

- **Claude Code**: supports image input via clipboard paste or file reference in prompts
- **Gemini CLI**: native multimodal input support including images, video, and audio
- **OpenAI**: Vision API supports images in Chat Completions and Responses API messages
- **Aider**: image support specifically for UI development workflows (screenshot → code)
- **Cursor/Windsurf**: IDE-integrated image attachment in chat panels

---

## 4. Gemini's Multimodal Capabilities

### Native Multimodal Processing

- Text, images, video, and audio are all first-class input modalities
- 1M+ token context window accommodates multimodal token counts
- Video understanding: analyze recorded code demos, tutorials, or bug reproductions
- Audio processing: transcribe and analyze voice recordings, meeting notes
- PDF and document understanding: process technical specifications directly

### Streaming with Multimodal Input

- Multimodal tokens (image, video, audio) affect time-to-first-token significantly
- Large video files may add 5-15 seconds of processing latency before streaming begins
- Streaming output is currently text-based regardless of input modality
- Future roadmap includes multimodal output (generated images, diagrams, audio)
- Token counting for multimodal inputs follows per-modality formulas

### Code-Specific Multimodal Uses

- Screenshot → CSS/HTML generation (pixel-accurate UI reproduction)
- Architecture diagram → code scaffolding and module structure
- Video of a bug → reproduction steps and potential fix code
- Whiteboard photo → algorithm implementation
- Design mockup → component hierarchy and styled code
- Error screenshot → debugging analysis and fix suggestions

---

## 5. Gemini Live API

### Overview

- WebSocket-based real-time multimodal streaming API
- Supports audio input and output in real-time bidirectional communication
- Tool use (function calling) during live sessions
- Session management with context preservation across turns
- Ephemeral tokens for client-side security (short-lived, scoped credentials)

### Technical Details

- Connection via `wss://` endpoint with model-specific paths
- Audio streamed as raw PCM chunks in both directions
- Server-side VAD for automatic turn detection
- Supports interruption: user can speak while model is generating
- Context window shared between audio and text tokens
- Session configuration includes voice selection, tools, and system instructions

### Comparison with OpenAI Realtime API

- Both use WebSocket for bidirectional audio streaming
- Gemini supports video input in live sessions (OpenAI does not yet)
- Both support function calling during voice conversations
- Different event schemas and session management approaches
- Gemini's larger context window allows longer multimodal sessions

---

## 6. Future: Video Streaming for Debugging

### Screen Sharing with Agents

- Stream terminal or IDE output to an agent in real-time
- Real-time code review: agent watches as you write and offers suggestions
- Agent observes a debugging session and suggests fixes as issues appear
- Potential: agent proactively helps by watching your coding patterns
- Reduces the need to manually describe context — the agent sees what you see

### Technical Challenges

- Video encoding/decoding latency adds to round-trip time
- Bandwidth requirements: even compressed screen capture is 1-5 Mbps
- Frame analysis computation: extracting text/UI from video frames
- Real-time response generation while processing continuous video input
- Privacy concerns: screen content may include sensitive information
- Cost: video tokens are expensive relative to text

### Emerging Approaches

- **Computer Use APIs** (Anthropic): agent controls mouse/keyboard on a virtual desktop
- **Screenshot-based interaction** (current state): periodic screenshots, not continuous video
- **Project Mariner** (Google): browser agent that navigates and interacts with web content
- Future: continuous video stream analysis with sub-second response latency
- Hybrid approaches: key-frame extraction rather than full video streaming

---

## 7. Text-to-Speech Output for Agents

### Current State

- Most coding agents produce text-only output in the terminal
- Some wrapper applications pipe LLM text responses through TTS engines
- OpenAI TTS API: `POST /v1/audio/speech` generates speech from text
- Alternative TTS providers: ElevenLabs, Google Cloud TTS, Azure Cognitive Services
- Streaming TTS: some APIs support chunked audio output for lower latency

### For Coding Agents

- Read code changes aloud while developer reviews visually
- Explain diffs and architectural decisions verbally
- Accessibility: essential for blind or low-vision developers using screen readers
- Hands-free notification of task completion or error status
- Background audio summaries while developer focuses on another task

### Technical Integration

- TTS can be applied post-hoc to any text stream
- Challenge: code syntax does not read well as speech (symbols, indentation)
- Need intelligent summarization layer between raw code output and TTS
- Latency budget: text generation + TTS synthesis + audio playback
- Voice selection and customization for developer preference

---

## 8. Accessibility Considerations

### Screen Reader Compatibility

- Terminal screen readers: NVDA (Windows), VoiceOver (macOS), Orca (Linux), JAWS
- ANSI escape codes (colors, cursor movement) can confuse screen readers
- Agents should provide semantic output alongside visual formatting
- `ACCESSIBILITY` or `NO_COLOR` environment variables signal preference for plain output
- Progress indicators should use text-based updates, not just spinner animations

### Motor Accessibility

- Voice input (Aider's `--voice` flag) reduces keyboard dependency
- Eye tracking integration for cursor control (future possibility)
- Switch access compatibility for users with limited motor function
- Reduced keyboard shortcut requirements — prefer simple commands
- Configurable confirmation modes (auto-approve for users who cannot easily type)

### Visual Accessibility

- High contrast modes for terminal output
- Configurable color schemes that respect system preferences
- Font size independence — terminal font configuration is separate from agent
- No color-only information conveyed — always include symbols or text labels
- Diff output should use `+`/`-` prefixes, not just red/green coloring

### Cognitive Accessibility

- Clear, predictable progress indicators (percentage, step counts)
- Predictable behavior: same input produces same workflow
- Undo/redo support for reverting agent-applied changes
- Reduced information overload: configurable verbosity levels
- Summaries before detailed output to help users decide what to read

---

## 9. Streaming Protocol Implications for Multimodal

### Voice Streaming Requirements

- **Bidirectional communication**: WebSocket required (SSE is server→client only)
- **Low latency target**: <200ms round-trip for conversational feel
- **Audio buffering**: handle jitter and network variability
- **VAD integration**: detect speech boundaries for turn-taking
- **Interruption handling**: user should be able to interrupt model speech
- **Codec negotiation**: client and server agree on audio format

### Multimodal Input Challenges

- Large payloads: images (100KB-5MB), video (10MB-500MB), audio (1MB-50MB)
- Base64 encoding overhead: ~33% size increase over binary
- Chunked upload for large files to avoid timeout and memory issues
- Separate upload endpoints with file references in prompts (preferred for large assets)
- Token counting complexity: different formulas for each modality

### Protocol Comparison for Multimodal

| Feature | SSE | WebSocket | WebRTC | WebTransport |
|---|---|---|---|---|
| Direction | Server→Client | Bidirectional | Peer-to-peer | Bidirectional |
| Audio support | No (text only) | Yes (binary frames) | Native | Yes |
| Latency | Medium | Low | Very low | Very low |
| NAT traversal | N/A | N/A | Yes (ICE/STUN) | N/A |
| Complexity | Low | Medium | High | Medium |
| Browser support | Excellent | Excellent | Good | Emerging |

### Future Protocol Directions

- **WebRTC**: peer-to-peer voice and video with minimal server involvement
- **WebTransport**: HTTP/3-based low-latency bidirectional streaming
- **QUIC**: underlying transport for reduced connection establishment latency
- **gRPC streaming**: bidirectional streaming with strong typing (protobuf)
- Trend: convergence toward single-connection multiplexed protocols
- Challenge: balancing latency, reliability, and implementation complexity

---

## 10. Integration Patterns

### Voice + Code Agent Pipeline

```
┌─────────┐    ┌───────────┐    ┌─────────┐    ┌──────────┐
│ Mic/Audio│───>│ Whisper / │───>│ LLM /   │───>│ Code Edit│
│ Capture  │    │ STT Engine│    │ Agent   │    │ & Apply  │
└─────────┘    └───────────┘    └─────────┘    └──────────┘
                                     │
                                     ▼
                               ┌──────────┐
                               │ TTS /    │───> Speaker
                               │ Voice Out│
                               └──────────┘
```

### Multimodal Context Assembly

```
User Input:
  ├── Text prompt: "Fix the layout bug shown in this screenshot"
  ├── Image: screenshot.png (base64-encoded, ~200KB)
  └── Code context: current file contents (auto-attached by agent)

→ Assembled into single API request
→ Response streams as text tokens (code fix + explanation)
```

### Latency Budget Breakdown (Voice Round-Trip)

```
Audio capture:           ~100ms  (utterance boundary detection)
Network upload:          ~50ms   (compressed audio payload)
Transcription (Whisper): ~1500ms (server-side processing)
LLM generation TTFT:     ~500ms (time to first token)
LLM streaming:           ~2000ms (full response)
TTS synthesis:           ~300ms  (if voice output enabled)
Audio playback start:    ~50ms
────────────────────────────────
Total:                   ~4500ms (end-to-end voice round-trip)
```

---

## Summary

Voice and multimodal streaming extend coding agents beyond text-only interaction.
The key technologies — Whisper for STT, Realtime/Live APIs for bidirectional voice,
Vision APIs for image understanding — each add latency and complexity but unlock
new interaction paradigms. The trajectory is clear: from text-only terminals toward
rich, multimodal, conversational coding experiences. Accessibility is both a
motivation and a beneficiary of these advances.
