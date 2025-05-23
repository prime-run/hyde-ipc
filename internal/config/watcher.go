package config

import (
	"log"
	"path/filepath"
	"sync"
	"time"

	"github.com/adrg/xdg"
	"github.com/fsnotify/fsnotify"

	"github.com/khing/hyde-ipc/internal/utils"
)

type ConfigWatcher struct {
	watcher    *fsnotify.Watcher
	configPath string
	onChange   func() error
	mutex      sync.Mutex
	lastEvent  time.Time
	cooldown   time.Duration
	verbose    bool
}

func NewConfigWatcher(onChange func() error, verbose bool) (*ConfigWatcher, error) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	configPath := filepath.Join(xdg.ConfigHome, "hyde")

	cw := &ConfigWatcher{
		watcher:    watcher,
		configPath: configPath,
		onChange:   onChange,
		lastEvent:  time.Now(),
		cooldown:   500 * time.Millisecond,
		verbose:    verbose,
	}

	return cw, nil
}

func (cw *ConfigWatcher) Start() error {

	if err := cw.watcher.Add(cw.configPath); err != nil {
		return err
	}

	go cw.watchLoop()
	return nil
}

func (cw *ConfigWatcher) watchLoop() {
	pending := false
	var timer *time.Timer

	for {
		select {
		case event, ok := <-cw.watcher.Events:
			if !ok {
				return
			}

			if filepath.Base(event.Name) == "config.toml" {

				if event.Op&(fsnotify.Write|fsnotify.Create) != 0 {
					cw.mutex.Lock()

					if !pending {
						pending = true
						if timer != nil {
							timer.Stop()
						}
						timer = time.AfterFunc(cw.cooldown, func() {
							cw.handleConfigChange()
							cw.mutex.Lock()
							pending = false
							cw.mutex.Unlock()
						})
					}

					cw.mutex.Unlock()
				}
			}

		case err, ok := <-cw.watcher.Errors:
			if !ok {
				return
			}
			log.Printf("Config watcher error: %v", err)
		}
	}
}

func (cw *ConfigWatcher) handleConfigChange() {
	utils.LogInfo("Config file changed, reloading...")

	err := cw.onChange()
	if err != nil {
		log.Printf("Failed to reload config: %v", err)
		log.Printf("Continuing with previous configuration")
	}
}

func (cw *ConfigWatcher) Close() error {
	return cw.watcher.Close()
}
