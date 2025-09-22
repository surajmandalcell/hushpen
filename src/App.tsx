import { useState } from "react";
import "./App.css";

// Components
import Sidebar from "./components/Sidebar";
import ContentArea from "./components/ContentArea";

function App() {
  const [activeSection, setActiveSection] = useState("general");

  return (
    <div className="app-container">
      <div className="main-window">
        <Sidebar 
          activeSection={activeSection} 
          onSectionChange={setActiveSection} 
        />
        <ContentArea activeSection={activeSection} />
      </div>
    </div>
  );
}

export default App;
