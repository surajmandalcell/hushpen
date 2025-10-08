import express from 'express';
import cors from 'cors';
import { createServer } from 'http';
import { Server as SocketIOServer } from 'socket.io';

// Routes will be imported here when implemented
// import apiRoutes from './routes';
// import { setupDockerService } from './services/dockerService';
// import { setupAudioService } from './services/audioService';
// import { setupConfigService } from './services/configService';

const app = express();
const server = createServer(app);
const io = new SocketIOServer(server, {
  cors: {
    origin: "*",
    methods: ["GET", "POST"]
  }
});

// Middleware
app.use(cors());
app.use(express.json());

// API Routes
// app.use('/api', apiRoutes);

// Health check endpoint
app.get('/health', (req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// WebSocket connection handling
io.on('connection', (socket) => {
  console.log('Client connected to WebSocket');
  
  socket.on('disconnect', () => {
    console.log('Client disconnected from WebSocket');
  });
  
  // WebSocket events will be handled here when implemented
  // socket.on('start-recording', async () => { ... });
  // socket.on('stop-recording', async () => { ... });
});

// Start server
const PORT = process.env.ELECTRON_BACKEND_PORT || 3001;
server.listen(PORT, () => {
  console.log(`Electron backend server running on port ${PORT}`);
});

// Initialize services (will be implemented when migrating backend)
// setupDockerService(io);
// setupAudioService(io);
// setupConfigService();

export { app, io };