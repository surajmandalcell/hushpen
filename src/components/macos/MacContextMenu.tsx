import React, { useState, useRef, useEffect } from "react";
import { ChevronRight } from "lucide-react";
import "./MacContextMenu.css";

export interface MacContextMenuItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  disabled?: boolean;
  separator?: boolean;
  submenu?: MacContextMenuItem[];
  onClick?: () => void;
}

export interface MacContextMenuProps {
  items: MacContextMenuItem[];
  isOpen: boolean;
  onClose: () => void;
  x: number;
  y: number;
  className?: string;
}

const MacContextMenu: React.FC<MacContextMenuProps> = ({
  items,
  isOpen,
  onClose,
  x,
  y,
  className = "",
}) => {
  const [hoveredItem, setHoveredItem] = useState<string | null>(null);
  const [openSubmenu, setOpenSubmenu] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        onClose();
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen, onClose]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen, onClose]);

  const handleItemClick = (item: MacContextMenuItem) => {
    if (item.disabled || item.separator) return;

    if (item.submenu) {
      setOpenSubmenu(openSubmenu === item.id ? null : item.id);
    } else {
      item.onClick?.();
      onClose();
    }
  };

  const handleItemHover = (item: MacContextMenuItem) => {
    if (item.disabled || item.separator) return;

    setHoveredItem(item.id);
    if (item.submenu) {
      setOpenSubmenu(item.id);
    } else {
      setOpenSubmenu(null);
    }
  };

  if (!isOpen) return null;

  const menuClasses = ["mac-context-menu", className].filter(Boolean).join(" ");

  return (
    <div
      ref={menuRef}
      className={menuClasses}
      style={{
        position: "fixed",
        left: x,
        top: y,
        zIndex: 1000,
      }}
    >
      <div className="mac-context-menu__content">
        {items.map((item, index) => {
          if (item.separator) {
            return <div key={`separator-${index}`} className="mac-context-menu__separator" />;
          }

          const isHovered = hoveredItem === item.id;
          const hasSubmenu = item.submenu && item.submenu.length > 0;
          const isSubmenuOpen = openSubmenu === item.id;

          return (
            <div key={item.id} className="mac-context-menu__item-container">
              <div
                className={[
                  "mac-context-menu__item",
                  item.disabled ? "mac-context-menu__item--disabled" : "",
                  isHovered ? "mac-context-menu__item--hovered" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => handleItemClick(item)}
                onMouseEnter={() => handleItemHover(item)}
                onMouseLeave={() => setHoveredItem(null)}
              >
                <div className="mac-context-menu__item-content">
                  {item.icon && (
                    <div className="mac-context-menu__item-icon">{item.icon}</div>
                  )}
                  <span className="mac-context-menu__item-label">{item.label}</span>
                  {hasSubmenu && (
                    <ChevronRight size={12} className="mac-context-menu__item-chevron" />
                  )}
                </div>
              </div>

              {hasSubmenu && isSubmenuOpen && (
                <div className="mac-context-menu__submenu">
                  {item.submenu!.map((subItem, subIndex) => {
                    if (subItem.separator) {
                      return (
                        <div
                          key={`sub-separator-${subIndex}`}
                          className="mac-context-menu__separator"
                        />
                      );
                    }

                    return (
                      <div
                        key={subItem.id}
                        className={[
                          "mac-context-menu__item",
                          subItem.disabled ? "mac-context-menu__item--disabled" : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        onClick={() => {
                          subItem.onClick?.();
                          onClose();
                        }}
                      >
                        <div className="mac-context-menu__item-content">
                          {subItem.icon && (
                            <div className="mac-context-menu__item-icon">{subItem.icon}</div>
                          )}
                          <span className="mac-context-menu__item-label">{subItem.label}</span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default MacContextMenu;
