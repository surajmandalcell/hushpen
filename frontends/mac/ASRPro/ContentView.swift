import SwiftUI

struct ContentView: View {
    @State private var transcriptionText = "Transcription results will appear here..."
    @State private var isTranscribing = false
    @State private var backendStatus = "Not connected"
    
    // Backend API integration will be implemented here
    // private let backendURL = URL(string: "http://localhost:8000")!
    
    var body: some View {
        VStack(spacing: 20) {
            Text("ASRPro")
                .font(.largeTitle)
                .fontWeight(.bold)
                .padding(.top, 20)
            
            Text("Automatic Speech Recognition for macOS")
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            Spacer()
            
            VStack(alignment: .leading, spacing: 10) {
                Text("Backend Status: \(backendStatus)")
                    .font(.caption)
                    .foregroundColor(backendStatus == "Connected" ? .green : .red)
                
                Button(action: {
                    // Backend API integration will be implemented here
                    transcribeAudio()
                }) {
                    HStack {
                        if isTranscribing {
                            ProgressView()
                                .scaleEffect(0.8)
                        } else {
                            Image(systemName: "mic.fill")
                        }
                        Text(isTranscribing ? "Transcribing..." : "Transcribe")
                    }
                    .frame(minWidth: 120)
                }
                .disabled(isTranscribing)
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
            }
            
            Spacer()
            
            VStack(alignment: .leading, spacing: 10) {
                Text("Transcription Results")
                    .font(.headline)
                
                ScrollView {
                    Text(transcriptionText)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding()
                        .background(Color(.controlBackgroundColor))
                        .cornerRadius(8)
                }
                .frame(minHeight: 200)
            }
            
            Spacer()
        }
        .padding(20)
        .frame(minWidth: 600, minHeight: 400)
        .onAppear(perform: checkBackendHealth)
    }
    
    private func transcribeAudio() {
        isTranscribing = true
        transcriptionText = "Preparing to transcribe audio..."
        
        // Backend API integration will be implemented here
        // This will connect to the backend API to transcribe audio files
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            transcriptionText = "Audio transcription functionality will be implemented here.\n\nThis will:\n1. Connect to the backend API at http://localhost:8000\n2. Send audio files for transcription\n3. Display the transcribed text\n\nMake sure the backend server is running before using this feature."
            isTranscribing = false
        }
    }
    
    private func checkBackendHealth() {
        // Backend health check will be implemented here
        // This will check if the backend API is accessible
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            backendStatus = "Not Connected"
            transcriptionText = "Backend health check will be implemented here.\n\nThe app will check if the backend server is running at http://localhost:8000 before attempting transcription."
        }
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}