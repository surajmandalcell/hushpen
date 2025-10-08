import express from 'express';
import cors from 'cors';
import { createServer } from 'http';
import { Server as SocketIOServer } from 'socket.io';

// Import services
import { setupConfigService, getConfigService } from './services/configService';
import { setupDockerService, getDockerService } from './services/dockerService';
import { setupAudioService, getAudioService } from './services/audioService';

// Import routes
import apiRoutes from './routes';

// Import utilities
import { logger } from './utils/logger';
import { errorHandler } from './utils/errors';

// Create Express app
const app = express();
const server = createServer(app);

// Create Socket.IO server
const io = new SocketIOServer(server, {
  cors: {
    origin: ["http://localhost:3000", "http://localhost:1420"],
    methods: ["GET", "POST"],
    credentials: true
  }
});

// Store io instance for use in routes
app.set('io', io);

// Middleware
app.use(cors({
  origin: ["http://localhost:3000", "http://localhost:1420"],
  credentials: true,
  methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
  allowedHeaders: ["Content-Type", "Authorization", "X-Requested-With"]
}));

app.use(express.json({ limit: '10mb' }));
app.use(express.urlencoded({ extended: true, limit: '10mb' }));

// Request logging middleware
app.use((req, res, next) => {
  logger.debug(`${req.method} ${req.path}`, {
    ip: req.ip,
    userAgent: req.get('User-Agent')
  });
  next();
});

// API Routes
app.use('/', apiRoutes);

// Root endpoint - redirect to API documentation
app.get('/', (req, res) => {
  res.json({
    message: 'ASR Pro API Server',
    version: '1.0.0',
    documentation: '/v1/options',
    endpoints: {
      health: '/health',
      models: '/v1/models',
      transcribe: '/v1/audio/transcriptions',
      options: '/v1/options'
    }
  });
});

// Error handling middleware (must be last)
app.use(errorHandler);

// WebSocket connection handling
const setupWebSocket = () => {
  io.on('connection', (socket) => {
    logger.info(`WebSocket client connected (total clients: ${io.engine.clientsCount})`);
    
    // Send initial system status to new client
    const sendSystemStatus = async () => {
      try {
        const dockerService = getDockerService();
        if (dockerService) {
          const systemStatus = await dockerService.getSystemStatus();
          socket.emit('system_status', systemStatus);
        }
      } catch (error) {
        logger.error('Failed to send system status to WebSocket client:', error);
      }
    };
    
    sendSystemStatus();
    
    // Handle incoming messages
    socket.on('message', async (data) => {
      try {
        const message = typeof data === 'string' ? JSON.parse(data) : data;
        logger.debug(`Received WebSocket message:`, message);
        
        switch (message.type) {
          case 'ping':
            socket.emit('pong', { timestamp: Date.now() });
            break;
            
          case 'get_status':
            await sendSystemStatus();
            break;
            
          default:
            logger.warn(`Unknown WebSocket message type: ${message.type}`);
        }
      } catch (error) {
        logger.error('Error handling WebSocket message:', error);
        socket.emit('error', { message: 'Invalid message format' });
      }
    });
    
    // Handle disconnection
    socket.on('disconnect', (reason) => {
      logger.info(`WebSocket client disconnected (reason: ${reason}, total clients: ${io.engine.clientsCount})`);
    });
    
    // Handle errors
    socket.on('error', (error) => {
      logger.error('WebSocket error:', error);
    });
  });
};

// Initialize services
const initializeServices = async () => {
  try {
    logger.info('Initializing ASR Pro backend services...');
    
    // Initialize configuration service
    const configService = setupConfigService();
    const serverConfig = configService.getServerConfig();
    logger.info(`Configuration service initialized with server config:`, serverConfig);
    
    // Initialize Docker service
    const dockerService = setupDockerService(io);
    await dockerService.initialize();
    logger.info('Docker service initialized');
    
    // Initialize audio service
    setupAudioService(io);
    logger.info('Audio service initialized');
    
    logger.info('All services initialized successfully');
  } catch (error) {
    logger.error('Failed to initialize services:', error);
    throw error;
  }
};

// Graceful shutdown
const gracefulShutdown = async () => {
  logger.info('Starting graceful shutdown...');
  
  try {
    // Close Socket.IO
    io.close(() => {
      logger.info('Socket.IO server closed');
    });
    
    // Close HTTP server
    server.close(() => {
      logger.info('HTTP server closed');
    });
    
    // Cleanup services
    const dockerService = getDockerService();
    if (dockerService) {
      await dockerService.cleanup();
      logger.info('Docker service cleaned up');
    }
    
    const audioService = getAudioService();
    if (audioService) {
      audioService.cleanup();
      logger.info('Audio service cleaned up');
    }
    
    logger.info('Graceful shutdown completed');
    process.exit(0);
  } catch (error) {
    logger.error('Error during graceful shutdown:', error);
    process.exit(1);
  }
};

// Handle process signals
process.on('SIGTERM', gracefulShutdown);
process.on('SIGINT', gracefulShutdown);

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
  logger.error('Uncaught exception:', error);
  gracefulShutdown();
});

// Handle unhandled rejections
process.on('unhandledRejection', (reason, promise) => {
  logger.error('Unhandled rejection at:', promise, 'reason:', reason);
  gracefulShutdown();
});

// Start server
const startServer = async () => {
  try {
    // Initialize services first
    await initializeServices();
    
    // Set up WebSocket
    setupWebSocket();
    
    // Get port from config or environment
    const configService = getConfigService();
    const serverConfig = configService.getServerConfig();
    const PORT = Number(process.env.ELECTRON_BACKEND_PORT) || serverConfig.port || 3001;
    const HOST = process.env.ELECTRON_BACKEND_HOST || serverConfig.host || '127.0.0.1';
    
    // Start listening
    server.listen(PORT, HOST, () => {
      logger.info(`ASR Pro backend server running on http://${HOST}:${PORT}`);
      logger.info(`WebSocket server available on ws://${HOST}:${PORT}`);
      logger.info(`API documentation available at http://${HOST}:${PORT}/v1/options`);
    });
    
  } catch (error) {
    logger.error('Failed to start server:', error);
    process.exit(1);
  }
};

// Start the server
startServer();

// Export for testing
export { app, io, server };