/// Zero-copy critical path manager and pipeline orchestration
use super::*;
use tokio::task::JoinHandle;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Critical path processing pipeline for zero-copy optimization
pub struct CriticalPath {
    /// Unique identifier for this path instance
    pub id: String,
    /// Allocation tracker for monitoring
    tracker: Arc<AllocationTracker>,
    /// Buffer pool for reuse
    buffer_pool: Arc<RwLock<BufferPool>>,
    /// Pipeline configuration
    config: CriticalPathConfig,
    /// Active processing contexts
    contexts: Arc<RwLock<HashMap<String, ProcessingContext>>>,
}

// Re-export buffer type so tests can import from manager path consistently
pub use super::ZeroCopyBuffer;

/// Configuration for critical path optimization
#[derive(Debug, Clone)]
pub struct CriticalPathConfig {
    /// Enable zero-copy optimizations
    pub enable_zero_copy: bool,
    /// Enable buffer pooling
    pub enable_buffer_pooling: bool,
    /// Maximum buffer pool size
    pub max_buffer_pool_size: usize,
    /// Buffer cleanup interval
    pub cleanup_interval: Duration,
    /// Maximum allocation tracking events
    pub max_tracking_events: usize,
    /// Enable detailed tracing
    pub enable_detailed_tracing: bool,
}

impl Default for CriticalPathConfig {
    fn default() -> Self {
        Self {
            enable_zero_copy: true,
            enable_buffer_pooling: true,
            max_buffer_pool_size: 1000,
            cleanup_interval: Duration::from_secs(30),
            max_tracking_events: 10000,
            enable_detailed_tracing: false,
        }
    }
}

/// Processing context for a single data flow
#[derive(Debug, Clone)]
pub struct ProcessingContext {
    /// Context identifier
    pub id: String,
    /// Current processing stage
    pub current_stage: Stage,
    /// Buffers managed by this context (simplified - removed due to ownership complexity)
    pub buffer_count: usize,
    /// Processing start time
    pub started_at: Instant,
    /// Stage transition history
    pub stage_history: Vec<(Stage, Instant)>,
}

impl ProcessingContext {
    pub fn new(id: String) -> Self {
        Self {
            id,
            current_stage: Stage::Crypto,
            buffer_count: 0,
            started_at: Instant::now(),
            stage_history: vec![(Stage::Crypto, Instant::now())],
        }
    }

    pub fn transition_to(&mut self, stage: Stage) {
        self.current_stage = stage;
        self.stage_history.push((stage, Instant::now()));
    }
}

impl CriticalPath {
    /// Create new critical path with specified configuration
    pub fn new(id: String, config: CriticalPathConfig) -> Self {
        let tracker = Arc::new(AllocationTracker::new(config.max_tracking_events));
        let buffer_pool = Arc::new(RwLock::new(BufferPool::new(
            config.max_buffer_pool_size / 10,
            config.max_buffer_pool_size,
        )));

        Self {
            id,
            tracker,
            buffer_pool,
            config,
            contexts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record an allocation event
    pub async fn record_allocation(&self, event: AllocationEvent) {
        self.tracker.record_allocation(event).await;
    }

    /// Get allocation metrics
    pub async fn get_metrics(&self) -> AllocationMetrics {
        self.tracker.get_metrics().await
    }

    /// Get path configuration
    pub fn get_config(&self) -> &CriticalPathConfig {
        &self.config
    }

    /// Start processing a packet through the critical path
    pub async fn start_processing(&self, context_id: String) -> Result<ProcessingContext, ZeroCopyError> {
        let mut contexts = self.contexts.write().await;
        
        if contexts.contains_key(&context_id) {
            return Err(ZeroCopyError::ContextAlreadyExists(context_id));
        }

        let context = ProcessingContext::new(context_id.clone());
        contexts.insert(context_id.clone(), context);

        // Record processing start
        self.tracker.record_allocation(AllocationEvent {
            stage: Stage::Crypto,
            operation: OperationType::ZeroCopy,
            size: 0,
            timestamp: Instant::now(),
            context: Some(context_id.clone()),
        }).await;

        Ok(contexts.get(&context_id).unwrap().clone())
    }

    /// Allocate buffer for specific stage with tracking
    pub async fn allocate_buffer(&self, context_id: &str, stage: Stage, size: usize) -> Result<ZeroCopyBuffer, ZeroCopyError> {
        self.tracker.start_timing();

        let buffer = if self.config.enable_buffer_pooling {
            // Try to get from pool first
            let mut pool = self.buffer_pool.write().await;
            let buffer = pool.get_buffer(size);
            
            self.tracker.record_allocation(AllocationEvent {
                stage,
                operation: OperationType::PoolGet,
                size,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            buffer
        } else {
            // Direct allocation
            self.tracker.record_allocation(AllocationEvent {
                stage,
                operation: OperationType::Allocate,
                size,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            ZeroCopyBuffer::new(size)
        };

        self.tracker.end_timing();

        // Update context
        let mut contexts = self.contexts.write().await;
        if let Some(_context) = contexts.get_mut(context_id) {
            // Note: We don't store the buffer in context to avoid ownership issues
            // The buffer will be managed by the caller
        }

        Ok(buffer)
    }

    /// Process data through crypto stage (AEAD)
    pub async fn process_crypto_stage(&self, context_id: &str, input_data: &[u8]) -> Result<ZeroCopyBuffer, ZeroCopyError> {
        // Transition context to crypto stage
        {
            let mut contexts = self.contexts.write().await;
            if let Some(context) = contexts.get_mut(context_id) {
                context.transition_to(Stage::Crypto);
            }
        }

        // Allocate output buffer (input size + AEAD tag overhead)
        let output_size = input_data.len() + 16; // ChaCha20Poly1305 tag size
        let mut output_buffer = self.allocate_buffer(context_id, Stage::Crypto, output_size).await?;

        // Simulate AEAD processing with zero-copy where possible
        if self.config.enable_zero_copy && input_data.len() <= output_buffer.capacity {
            // Zero-copy: direct write to output buffer
            output_buffer.as_mut().extend_from_slice(input_data);
            output_buffer.as_mut().extend_from_slice(&[0u8; 16]); // Mock AEAD tag

            self.tracker.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::ZeroCopy,
                size: input_data.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;
        } else {
            // Fallback to copy operation
            self.tracker.start_copy_timing();
            output_buffer.as_mut().clear();
            output_buffer.as_mut().extend_from_slice(input_data);
            output_buffer.as_mut().extend_from_slice(&[0u8; 16]);
            self.tracker.end_copy_timing();

            self.tracker.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::Copy,
                size: input_data.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;
        }

        debug!("Processed {} bytes through crypto stage for context {}", input_data.len(), context_id);
        Ok(output_buffer)
    }

    /// Process data through FEC stage (RaptorQ)
    pub async fn process_fec_stage(&self, context_id: &str, input_buffer: &ZeroCopyBuffer) -> Result<Vec<ZeroCopyBuffer>, ZeroCopyError> {
        // Transition context to FEC stage
        {
            let mut contexts = self.contexts.write().await;
            if let Some(context) = contexts.get_mut(context_id) {
                context.transition_to(Stage::Fec);
            }
        }

        let input_data = input_buffer.as_ref();
        
        // Calculate RaptorQ encoding parameters
        let symbol_size = 1280; // RaptorQ symbol size from nyx-fec
        let num_symbols = (input_data.len() + symbol_size - 1) / symbol_size;
        let redundancy_symbols = (num_symbols as f32 * 0.3) as usize; // 30% redundancy
        let total_symbols = num_symbols + redundancy_symbols;

        let mut output_buffers = Vec::with_capacity(total_symbols);

        // Encode symbols with zero-copy optimization
        for symbol_idx in 0..total_symbols {
            let symbol_buffer = self.allocate_buffer(context_id, Stage::Fec, symbol_size).await?;
            
            if symbol_idx < num_symbols {
                // Source symbol - try zero-copy
                let start_offset = symbol_idx * symbol_size;
                let end_offset = std::cmp::min(start_offset + symbol_size, input_data.len());
                let symbol_data = &input_data[start_offset..end_offset];

                if self.config.enable_zero_copy {
                    // Zero-copy reference to input data
                    self.tracker.record_allocation(AllocationEvent {
                        stage: Stage::Fec,
                        operation: OperationType::ZeroCopy,
                        size: symbol_data.len(),
                        timestamp: Instant::now(),
                        context: Some(context_id.to_string()),
                    }).await;
                } else {
                    // Copy operation
                    self.tracker.start_copy_timing();
                    self.tracker.record_allocation(AllocationEvent {
                        stage: Stage::Fec,
                        operation: OperationType::Copy,
                        size: symbol_data.len(),
                        timestamp: Instant::now(),
                        context: Some(context_id.to_string()),
                    }).await;
                    self.tracker.end_copy_timing();
                }
            } else {
                // Repair symbol - requires computation
                self.tracker.record_allocation(AllocationEvent {
                    stage: Stage::Fec,
                    operation: OperationType::Allocate,
                    size: symbol_size,
                    timestamp: Instant::now(),
                    context: Some(context_id.to_string()),
                }).await;
            }

            output_buffers.push(symbol_buffer);
        }

        debug!("Processed {} symbols ({} source + {} repair) through FEC stage for context {}", 
               total_symbols, num_symbols, redundancy_symbols, context_id);
        
        Ok(output_buffers)
    }

    /// Process data through transmission stage
    pub async fn process_transmission_stage(&self, context_id: &str, symbol_buffers: &[ZeroCopyBuffer]) -> Result<Vec<ZeroCopyBuffer>, ZeroCopyError> {
        // Transition context to transmission stage
        {
            let mut contexts = self.contexts.write().await;
            if let Some(context) = contexts.get_mut(context_id) {
                context.transition_to(Stage::Transmission);
            }
        }

        let mut transmission_buffers = Vec::with_capacity(symbol_buffers.len());

        for (_idx, symbol_buffer) in symbol_buffers.iter().enumerate() {
            let symbol_data = symbol_buffer.as_ref();
            
            // Add transmission headers (simulated)
            let header_size = 32; // IP + UDP + Nyx headers
            let total_size = header_size + symbol_data.len();
            
            let mut tx_buffer = self.allocate_buffer(context_id, Stage::Transmission, total_size).await?;

            if self.config.enable_zero_copy && symbol_buffer.can_reuse() {
                // Zero-copy: reference existing buffer
                self.tracker.record_allocation(AllocationEvent {
                    stage: Stage::Transmission,
                    operation: OperationType::ZeroCopy,
                    size: symbol_data.len(),
                    timestamp: Instant::now(),
                    context: Some(context_id.to_string()),
                }).await;

                // Only copy headers, reference payload
                let mock_headers = vec![0u8; header_size];
                tx_buffer.as_mut().extend_from_slice(&mock_headers);
            } else {
                // Copy operation required
                self.tracker.start_copy_timing();
                let mock_headers = vec![0u8; header_size];
                tx_buffer.as_mut().extend_from_slice(&mock_headers);
                tx_buffer.as_mut().extend_from_slice(symbol_data);
                self.tracker.end_copy_timing();

                self.tracker.record_allocation(AllocationEvent {
                    stage: Stage::Transmission,
                    operation: OperationType::Copy,
                    size: symbol_data.len(),
                    timestamp: Instant::now(),
                    context: Some(context_id.to_string()),
                }).await;
            }

            transmission_buffers.push(tx_buffer);
        }

        debug!("Processed {} transmission buffers for context {}", transmission_buffers.len(), context_id);
        Ok(transmission_buffers)
    }

    /// Process complete packet through entire critical path
    pub async fn process_packet(&self, input_data: &[u8]) -> Result<Vec<ZeroCopyBuffer>, ZeroCopyError> {
        let context_id = format!("packet_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        
        // Start processing
        let _context = self.start_processing(context_id.clone()).await?;

        // Crypto stage
        let crypto_output = self.process_crypto_stage(&context_id, input_data).await?;

        // FEC stage  
        let fec_outputs = self.process_fec_stage(&context_id, &crypto_output).await?;

        // Transmission stage
        let tx_outputs = self.process_transmission_stage(&context_id, &fec_outputs).await?;

        // Cleanup context
        self.finish_processing(&context_id).await?;

        Ok(tx_outputs)
    }

    /// Finish processing and return buffers to pool
    pub async fn finish_processing(&self, context_id: &str) -> Result<(), ZeroCopyError> {
        let mut contexts = self.contexts.write().await;
        
        if let Some(context) = contexts.remove(context_id) {
            // Return buffers to pool (simplified - no actual buffers stored)
            if self.config.enable_buffer_pooling {
                // Buffers would be returned here if we tracked them
                    
                // Record pool return event (even though simplified)
                self.tracker.record_allocation(AllocationEvent {
                    stage: context.current_stage,
                    operation: OperationType::PoolReturn,
                    size: 0,
                    timestamp: Instant::now(),
                    context: Some(context_id.to_string()),
                }).await;
            }

            let duration = context.started_at.elapsed();
            debug!("Finished processing context {} in {:?}", context_id, duration);
        }

        Ok(())
    }

    /// Get buffer pool statistics
    pub async fn get_pool_stats(&self) -> BufferPoolStats {
        self.buffer_pool.read().await.stats()
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(&self) -> JoinHandle<()> {
        let buffer_pool = Arc::clone(&self.buffer_pool);
        let cleanup_interval = self.config.cleanup_interval;
        let max_age = Duration::from_secs(300); // 5 minutes

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                {
                    let mut pool = buffer_pool.write().await;
                    pool.cleanup(max_age);
                }
            }
        })
    }
}

/// Zero-copy manager for coordinating multiple critical paths
pub struct ZeroCopyManager {
    /// Active critical paths
    paths: Arc<RwLock<HashMap<String, Arc<CriticalPath>>>>,
    /// Global configuration
    config: ZeroCopyManagerConfig,
    /// Background tasks
    cleanup_tasks: Vec<JoinHandle<()>>,
}

/// Configuration for zero-copy manager
#[derive(Debug, Clone)]
pub struct ZeroCopyManagerConfig {
    /// Default critical path configuration
    pub default_path_config: CriticalPathConfig,
    /// Maximum number of active paths
    pub max_active_paths: usize,
    /// Global cleanup interval
    pub global_cleanup_interval: Duration,
    /// Enable metrics aggregation
    pub enable_metrics_aggregation: bool,
}

impl Default for ZeroCopyManagerConfig {
    fn default() -> Self {
        Self {
            default_path_config: CriticalPathConfig::default(),
            max_active_paths: 1000,
            global_cleanup_interval: Duration::from_secs(60),
            enable_metrics_aggregation: true,
        }
    }
}

impl ZeroCopyManager {
    /// Create new zero-copy manager
    pub fn new(config: ZeroCopyManagerConfig) -> Self {
        Self {
            paths: Arc::new(RwLock::new(HashMap::new())),
            config,
            cleanup_tasks: Vec::new(),
        }
    }

    /// Create new critical path
    pub async fn create_critical_path(&self, path_id: String) -> Result<Arc<CriticalPath>, ZeroCopyError> {
        let mut paths = self.paths.write().await;
        
        // Prefer duplicate path detection over capacity limit for clearer error semantics
        if paths.contains_key(&path_id) {
            return Err(ZeroCopyError::PathAlreadyExists(path_id));
        }

        if paths.len() >= self.config.max_active_paths {
            return Err(ZeroCopyError::TooManyPaths);
        }

        let path = Arc::new(CriticalPath::new(path_id.clone(), self.config.default_path_config.clone()));
        paths.insert(path_id.clone(), Arc::clone(&path));

        info!("Created critical path: {}", path_id);
        Ok(path)
    }

    /// Get existing critical path
    pub async fn get_critical_path(&self, path_id: &str) -> Option<Arc<CriticalPath>> {
        let paths = self.paths.read().await;
        paths.get(path_id).cloned()
    }

    /// Remove critical path
    pub async fn remove_critical_path(&self, path_id: &str) -> Result<(), ZeroCopyError> {
        let mut paths = self.paths.write().await;
        
        if paths.remove(path_id).is_some() {
            info!("Removed critical path: {}", path_id);
            Ok(())
        } else {
            Err(ZeroCopyError::PathNotFound(path_id.to_string()))
        }
    }

    /// Get aggregated metrics across all paths
    pub async fn get_aggregated_metrics(&self) -> AggregatedMetrics {
        let paths = self.paths.read().await;
        let mut aggregated = AggregatedMetrics::default();

        for path in paths.values() {
            let metrics = path.get_metrics().await;
            aggregated.merge(metrics);
        }

        aggregated
    }

    /// Start background management tasks
    pub async fn start_background_tasks(&mut self) {
        // Global cleanup task
        let paths = Arc::clone(&self.paths);
        let cleanup_interval = self.config.global_cleanup_interval;
        
        let cleanup_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                let paths_guard = paths.read().await;
                for _path in paths_guard.values() {
                    // Individual path cleanup is handled by each path's cleanup task
                }
            }
        });

        self.cleanup_tasks.push(cleanup_task);
    }
}

/// Aggregated metrics across multiple critical paths
#[derive(Debug, Default, Clone)]
pub struct AggregatedMetrics {
    pub total_paths: usize,
    pub combined_allocations: u64,
    pub combined_bytes: u64,
    pub average_zero_copy_ratio: f64,
    pub average_reduction_ratio: f64,
    pub total_allocation_overhead_ns: u64,
    pub per_path_metrics: HashMap<String, AllocationMetrics>,
}

impl AggregatedMetrics {
    pub fn merge(&mut self, metrics: AllocationMetrics) {
        self.combined_allocations += metrics.pipeline_total_allocations;
        self.combined_bytes += metrics.pipeline_total_bytes;
        self.total_allocation_overhead_ns += metrics.allocation_overhead_ns;
        
        // Update averages
        let current_paths = self.total_paths as f64;
        self.average_zero_copy_ratio = (self.average_zero_copy_ratio * current_paths + metrics.zero_copy_ratio) / (current_paths + 1.0);
        self.average_reduction_ratio = (self.average_reduction_ratio * current_paths + metrics.reduction_ratio) / (current_paths + 1.0);
        
        self.total_paths += 1;
    }
}

/// Zero-copy optimization errors
#[derive(Debug, thiserror::Error)]
pub enum ZeroCopyError {
    #[error("Context already exists: {0}")]
    ContextAlreadyExists(String),
    
    #[error("Path already exists: {0}")]
    PathAlreadyExists(String),
    
    #[error("Path not found: {0}")]
    PathNotFound(String),
    
    #[error("Too many active paths")]
    TooManyPaths,
    
    #[error("Buffer allocation failed")]
    AllocationFailed,
    
    #[error("Invalid buffer operation")]
    InvalidOperation,
}
