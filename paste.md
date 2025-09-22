<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Select Box</title>
<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  background: #121212;
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
}

.select-container {
  position: relative;
  width: 320px;
}

.select-box {
  width: 100%;
  padding: 14px 50px 14px 24px;
  background: rgba(40,40,40,0.9);
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 12px;
  color: #fff;
  font-size: 18px;
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.arrow {
  position: absolute;
  right: 20px;
}

.dropdown {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  right: 0;
  background: rgba(40,40,40,0.95);
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 12px;
  overflow: hidden;
  opacity: 0;
  transform: translateY(-10px);
  pointer-events: none;
  transition: all 0.2s;
}

.dropdown.active {
  opacity: 1;
  transform: translateY(0);
  pointer-events: all;
}

.option {
  padding: 14px 24px;
  color: rgba(255,255,255,0.8);
  cursor: pointer;
  transition: all 0.15s;
  font-size: 18px;
}

.option:hover {
  background: rgba(255,255,255,0.08);
  color: #fff;
}

.option.selected {
  color: #fff;
  background: rgba(255,255,255,0.05);
}
</style>
</head>
<body>
<div class="select-container">
  <div class="select-box">
    <span>Cupertino</span>
    <svg class="arrow" width="12" height="8" viewBox="0 0 12 8" fill="none">
      <path d="M1 1.5L6 6.5L11 1.5" stroke="rgba(255,255,255,0.7)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </div>
  <div class="dropdown active">
    <div class="option selected">Cupertino</div>
    <div class="option">Chicago</div>
    <div class="option">Monaco</div>
    <div class="option">San Francisco</div>
    <div class="option">New York</div>
  </div>
</div>
</body>
</html>